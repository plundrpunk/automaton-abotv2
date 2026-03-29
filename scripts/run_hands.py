#!/usr/bin/env python3
"""Launch shipped ABot v3 hands against the current single-agent runtime."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import time
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None

DEFAULT_HANDS = [
    "general-assistant",
    "researcher",
    "backend-engineer",
    "frontend-engineer",
    "technical-writer",
    "memory-curator",
    "data-analyst",
    "task-runner",
]

REPO_ROOT = Path(__file__).resolve().parents[2]
ABOT_ROOT = REPO_ROOT / "abot-v3"
DEFAULT_CONFIG = ABOT_ROOT / "config" / "abot.toml"
DEFAULT_HANDS_DIR = ABOT_ROOT / "hands"
DEFAULT_BINARY_CANDIDATES = [
    ABOT_ROOT / "target" / "release" / "abot",
    ABOT_ROOT / "target" / "debug" / "abot",
]


def load_hand_manifest(hand_name: str, hands_dir: Path) -> dict:
    manifest_path = hands_dir / hand_name / "HAND.toml"
    if not manifest_path.exists():
        raise SystemExit(f"Unknown hand: {hand_name}")
    if tomllib is None:
        return {"hand": {"name": hand_name, "description": ""}}
    with manifest_path.open("rb") as handle:
        return tomllib.load(handle)


def resolve_hand_names(hands_dir: Path) -> list[str]:
    names: list[str] = []
    for hand_name in DEFAULT_HANDS:
        if (hands_dir / hand_name / "HAND.toml").exists():
            names.append(hand_name)
    return names


def resolve_binary(explicit_binary: str | None, force_cargo: bool) -> list[str]:
    if explicit_binary:
        binary_path = Path(explicit_binary).expanduser().resolve()
        if not binary_path.exists():
            raise SystemExit(f"Binary not found: {binary_path}")
        return [str(binary_path)]
    if not force_cargo:
        for candidate in DEFAULT_BINARY_CANDIDATES:
            if candidate.exists() and os.access(candidate, os.X_OK):
                return [str(candidate)]
    return ["cargo", "run", "-p", "abot-cli", "--"]


def build_agent_command(
    base_command: list[str],
    config_path: Path,
    hand_name: str,
    log_level: str,
    extra_args: list[str],
) -> list[str]:
    command = list(base_command)
    command.extend(
        [
            "--config",
            str(config_path),
            "--agent-name",
            hand_name,
            "--agent-id",
            hand_name,
            "--log-level",
            log_level,
        ]
    )
    command.extend(extra_args)
    return command


def terminate_process(proc: subprocess.Popen, graceful: bool = True) -> None:
    if proc.poll() is not None:
        return
    try:
        if os.name == "posix":
            sig = signal.SIGTERM if graceful else signal.SIGKILL
            os.killpg(proc.pid, sig)
        elif graceful:
            proc.terminate()
        else:
            proc.kill()
    except ProcessLookupError:
        pass


def launch_one(hand_name: str, args: argparse.Namespace) -> int:
    hands_dir = Path(args.hands_dir).resolve()
    config_path = Path(args.config).resolve()
    load_hand_manifest(hand_name, hands_dir)
    command = build_agent_command(
        resolve_binary(args.binary, args.cargo),
        config_path,
        hand_name,
        args.log_level,
        args.runtime_args,
    )
    env = os.environ.copy()
    env["AUTOMATON_AGENT_NAME"] = hand_name
    env["AUTOMATON_AGENT_ID"] = hand_name
    print(f"[run] {hand_name}: {' '.join(command)}")
    completed = subprocess.run(command, cwd=ABOT_ROOT, env=env)
    return completed.returncode


def terminate_process_tree(
    processes: list[subprocess.Popen], grace_period: float = 5.0
) -> None:
    active = [proc for proc in processes if proc.poll() is None]
    if not active:
        return
    for proc in active:
        terminate_process(proc, graceful=True)
    deadline = time.time() + grace_period
    while time.time() < deadline:
        if all(proc.poll() is not None for proc in active):
            return
        time.sleep(0.1)
    for proc in active:
        if proc.poll() is None:
            terminate_process(proc, graceful=False)


def launch_all(args: argparse.Namespace) -> int:
    hands_dir = Path(args.hands_dir).resolve()
    config_path = Path(args.config).resolve()
    hand_names = resolve_hand_names(hands_dir)
    if not hand_names:
        raise SystemExit(f"No shipped hands found in {hands_dir}")
    base_command = resolve_binary(args.binary, args.cargo)
    processes: list[subprocess.Popen] = []
    interrupted = False

    def handle_signal(signum, _frame):
        nonlocal interrupted
        interrupted = True
        print(
            f"\n[signal] received {signal.Signals(signum).name}, terminating child processes..."
        )
        terminate_process_tree(processes)

    previous_sigint = signal.signal(signal.SIGINT, handle_signal)
    previous_sigterm = signal.signal(signal.SIGTERM, handle_signal)
    try:
        for hand_name in hand_names:
            load_hand_manifest(hand_name, hands_dir)
            command = build_agent_command(
                base_command, config_path, hand_name, args.log_level, args.runtime_args
            )
            env = os.environ.copy()
            env["AUTOMATON_AGENT_NAME"] = hand_name
            env["AUTOMATON_AGENT_ID"] = hand_name
            print(f"[spawn] {hand_name}: {' '.join(command)}")
            processes.append(
                subprocess.Popen(
                    command, cwd=ABOT_ROOT, env=env, start_new_session=True
                )
            )
        exit_code = 0
        while True:
            running = [proc for proc in processes if proc.poll() is None]
            if not running:
                break
            if interrupted:
                exit_code = 130
                break
            for proc in processes:
                returncode = proc.poll()
                if returncode not in (None, 0):
                    print(
                        f"[exit] child pid={proc.pid} returned {returncode}, terminating remaining processes..."
                    )
                    terminate_process_tree(processes)
                    return returncode
            time.sleep(0.25)
        if interrupted:
            return 130
        return exit_code
    finally:
        signal.signal(signal.SIGINT, previous_sigint)
        signal.signal(signal.SIGTERM, previous_sigterm)
        terminate_process_tree(processes)


def list_hands(args: argparse.Namespace) -> int:
    hands_dir = Path(args.hands_dir).resolve()
    names = resolve_hand_names(hands_dir)
    if not names:
        print(f"No shipped hands found in {hands_dir}")
        return 1
    for hand_name in names:
        manifest = load_hand_manifest(hand_name, hands_dir)
        description = manifest.get("hand", {}).get("description", "")
        print(f"{hand_name}	{description}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Launch shipped ABot v3 hands")
    parser.add_argument(
        "--config", default=str(DEFAULT_CONFIG), help="Path to abot.toml"
    )
    parser.add_argument(
        "--hands-dir",
        default=str(DEFAULT_HANDS_DIR),
        help="Path to shipped hands directory",
    )
    parser.add_argument("--log-level", default="info", help="ABot runtime log level")
    parser.add_argument("--binary", help="Path to a compiled abot binary")
    parser.add_argument(
        "--cargo",
        action="store_true",
        help="Force cargo run even if a compiled binary exists",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    list_parser = subparsers.add_parser("list", help="List shipped bodies")
    list_parser.set_defaults(func=list_hands)

    run_parser = subparsers.add_parser("run", help="Run one shipped body")
    run_parser.add_argument("hand_name", choices=DEFAULT_HANDS, help="Hand name to run")
    run_parser.add_argument(
        "runtime_args",
        nargs=argparse.REMAINDER,
        help="Extra args passed to abot after --",
    )
    run_parser.set_defaults(func=lambda parsed: launch_one(parsed.hand_name, parsed))

    run_all_parser = subparsers.add_parser("run-all", help="Run all shipped bodies")
    run_all_parser.add_argument(
        "runtime_args",
        nargs=argparse.REMAINDER,
        help="Extra args passed to each abot process after --",
    )
    run_all_parser.set_defaults(func=launch_all)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if (
        hasattr(args, "runtime_args")
        and args.runtime_args
        and args.runtime_args[0] == "--"
    ):
        args.runtime_args = args.runtime_args[1:]
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
