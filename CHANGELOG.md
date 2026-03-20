# Changelog

All notable changes to Automaton Abot will be documented in this file.

## [2.0.0] - 2026-03-19

### Added
- Open-source release of the Automaton Abot orchestrator
- Persistent container pool with session continuity
- Embedded Warden with death/birth ritual lifecycle management
- AMS Fleet protocol (heartbeat, execution streaming, message polling)
- AMS Communications Gateway channel (Telegram, Discord, Slack, WhatsApp)
- Task scheduler with cron, interval, and one-shot scheduling
- Mount security with tamper-proof host directory allowlists
- Observatory hooks for real-time fleet telemetry
- MCP bridge (SSE→stdio) for exposing AMS tools inside containers
- Skills system with SKILL.md sync to agent sessions
- Docker networking for AMS service discovery inside containers
- Health check server for container orchestration
- Dashboard message routing with fleet daemon detection
