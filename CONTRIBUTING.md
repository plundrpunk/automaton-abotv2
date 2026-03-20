# Contributing to Automaton Abot

Thanks for your interest in contributing! Here's how to get started.

## Development Setup

1. Fork the repo and clone your fork
2. Install dependencies: `npm install`
3. Build the agent container: `npm run docker:build`
4. Copy `.env.example` to `.env` and configure
5. Run in dev mode: `npm run dev`

## Code Style

- TypeScript strict mode
- Prettier for formatting (`npm run format`)
- Meaningful commit messages (conventional commits preferred)

## Pull Requests

1. Create a feature branch from `master`
2. Make your changes
3. Run `npm run typecheck` — must pass with zero errors
4. Run `npm run format:check` — must pass
5. Run `npm test` — must pass
6. Open a PR with a clear description of what changed and why

## Architecture

The codebase has two layers:

- **Host daemon** (`src/`) — runs on the host machine, manages containers, handles fleet protocol
- **Agent runner** (`container/agent-runner/`) — runs inside each Docker container, executes Claude Code queries

Changes to the agent runner require rebuilding the Docker image (`npm run docker:build`).

## Reporting Issues

Open a GitHub issue with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Relevant logs (sanitize any credentials)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
