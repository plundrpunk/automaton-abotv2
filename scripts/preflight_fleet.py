#!/usr/bin/env python3
"""Validate the PR #64 source model without a daemon, credentials, or rollout.

Requires Python 3.11+ and Docker Compose. Only invokes `compose config` and
read-only Git commands. Exit 0 means source checks passed, not deploy ready.
"""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib


PRIME = "automaton-abot-prime-v2"
LEADS = {
    f"tl-{name}" for name in (
        "academic", "design", "engineering", "finance", "gamedev", "gis",
        "healthcare", "marketing", "paid-media", "product", "project-mgmt",
        "sales", "security", "spatial", "specialized", "support", "testing",
    )
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def render(root, mount=None):
    # Do not load .env, inherited COMPOSE_* controls, or real AMS credentials.
    env = {key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}
    env.update(AUTOMATON_AMS_API_KEY="", AUTOMATON_AMS_URL="http://ams-server:3001")
    if mount is not None:
        env["AUTOMATON_HOST_AMS_DIR"] = mount
    result = subprocess.run(
        ["docker", "compose", "--env-file", os.devnull, "-p", "abot-preflight",
         "-f", str(root / "docker-compose.hands.yml"), "config", "--format", "json"],
        cwd=root, env=env, text=True, capture_output=True, timeout=30,
    )
    # Do not echo raw Compose output: it could contain interpolated values.
    require(result.returncode == 0, "Compose model could not be rendered")
    return json.loads(result.stdout)


def validate_model(root, model, mount):
    services = model["services"]
    require(set(services) == LEADS | {PRIME}, "Expected Prime and exactly 17 named TL services")
    network = model["networks"]["ams_network"]
    require(network.get("external") is True and network["name"] == "ams_ams_network",
            "Expected external ams_ams_network")
    rows = []
    for name, service in sorted(services.items()):
        env = service["environment"]
        identity = "Automaton-Abot Prime V2" if name == PRIME else name
        require(env.get("AUTOMATON_AGENT_NAME") == env.get("AUTOMATON_AGENT_ID") == identity,
                f"{name}: incorrect agent identity")
        directory = root / env.get("AUTOMATON_HAND_DIR", f"hands/{identity}")
        require(directory.resolve() == (root / "hands" / name).resolve(),
                f"{name}: incorrect hand directory (Prime needs its explicit override)")
        manifest = tomllib.loads((directory / "HAND.toml").read_text())
        hand, matching = manifest["hand"], manifest["matching"]
        require(hand["name"] == hand["agent_id"] == matching["seed_name"] == identity,
                f"{name}: manifest or seeded-head mismatch")
        require(matching["requires_seeded_head"] is True, f"{name}: seeded head is required")
        for field in ("skill_file", "system_prompt_file"):
            asset = (directory / hand[field]).resolve()
            require(asset.is_relative_to(directory.resolve()) and asset.is_file()
                    and asset.stat().st_size > 0, f"{name}: missing or invalid {field}")
        require(hand["default_model"] == "codex", f"{name}: unexpected model routing")
        require(service["image"] == "abot-v3:local", f"{name}: inconsistent image")
        require(Path(service["build"]["context"]).resolve() == root
                and service["build"]["dockerfile"] == "Dockerfile", f"{name}: unexpected build")
        require(service["command"] == ["abot", "--config", "config/abot.toml"],
                f"{name}: unexpected command")
        require(service["restart"] == "unless-stopped", f"{name}: missing restart policy")
        require(set(service["networks"]) == {"ams_network"}, f"{name}: incorrect network")
        require(not service.get("profiles") and not service.get("ports"),
                f"{name}: unexpected profile or published port")
        require(env.get("AUTOMATON_AMS_URL") == "http://ams-server:3001"
                and env.get("AUTOMATON_AMS_API_KEY") == "", f"{name}: unexpected AMS settings")
        volumes = service.get("volumes", [])
        require(len(volumes) == 1 and volumes[0].get("type") == "bind"
                and volumes[0].get("source") == mount
                and volumes[0].get("target") == "/home/andrew/ams"
                and volumes[0].get("read_only") is True,
                f"{name}: expected one read-only AMS bind, no hand overlays")
        rows.append({"service": name, "agent_id": identity,
                     "hand_directory": f"hands/{name}",
                     "archetype": hand["archetype"],
                     "healthcheck_defined": bool(service.get("healthcheck"))})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    root = parser.parse_args().root.resolve()
    try:
        rows = validate_model(root, render(root), str(root / "ams"))
        validate_model(root, render(root, "/home/andrew/ams"), "/home/andrew/ams")
        dockerfile = (root / "Dockerfile").read_text()
        compiler = re.search(r"^FROM rust:(\d+)\.(\d+)(?:\.\d+)?-bookworm AS builder$",
                             dockerfile, re.MULTILINE)
        require(compiler is not None and tuple(map(int, compiler.groups())) >= (1, 88),
                "Docker builder requires Rust >= 1.88 for existing Rust 2024 let chains")
        require("RUN cargo build --release --locked -p abot-cli" in dockerfile,
                "Docker build must enforce Cargo.lock")
        require("COPY hands ./hands" in dockerfile and "COPY config ./config" in dockerfile,
                "Image must contain hand assets and configuration")
        ignores = (root / "Dockerfile.dockerignore").read_text().splitlines()
        require(all(value in ignores for value in (".git", "target", ".env", ".env.*")),
                "Source build context must exclude Git, Cargo output and environment files")
        config = tomllib.loads((root / "config/abot.toml").read_text())
        require(config["hands"]["directory"] == "./hands", "Unexpected configured hand directory")
        revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, text=True,
                                  capture_output=True, check=True, timeout=10).stdout.strip()
        report = {
            "status": "PASS", "scope": "source model only", "rollout_ready": False,
            "source_revision": revision, "services": rows,
            "mount_variants_checked": ["default ./ams", "explicit /home/andrew/ams"],
            "heartbeat_interval_secs": config["ams"]["heartbeat_interval_secs"],
            "local_checkout_free_gib": round(shutil.disk_usage(root).free / 2**30, 1),
            "unverified": ["Linux release image build", "VPS disk and build capacity",
                           "live external network and AMS checkout bind",
                           "AMS credentials, seeded heads and grants",
                           "live Warden/fleet health and tool execution",
                           "rollback image ID and VPS backup bundle"],
        }
        sys.stdout.write(json.dumps(report, indent=2) + "\n")
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        sys.stdout.write(json.dumps({"status": "FAIL", "rollout_ready": False,
                                    "error": str(error)}) + "\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
