#!/bin/bash
set -e

ABOT_BIN="/home/andrew/automaton-abotv2/target/release/abot"
CONFIG="/home/andrew/automaton-abotv2/config/abot.toml"
AMS_URL="http://localhost:3001"
LOG_DIR="/home/andrew/automaton-abotv2/logs"
PID_DIR="/home/andrew/automaton-abotv2/pids"

mkdir -p "$LOG_DIR" "$PID_DIR"

BODIES=(general-assistant researcher backend-engineer frontend-engineer technical-writer memory-curator data-analyst task-runner)

case "${1:-start}" in
  start)
    for name in "${BODIES[@]}"; do
      if [ -f "$PID_DIR/$name.pid" ] && kill -0 "$(cat "$PID_DIR/$name.pid")" 2>/dev/null; then
        echo "[SKIP] $name already running (PID $(cat "$PID_DIR/$name.pid"))"
        continue
      fi
      echo "Starting $name..."
      AUTOMATON_AMS_URL="$AMS_URL" \
      AUTOMATON_AGENT_NAME="$name" \
      AUTOMATON_AGENT_ID="$name" \
      RUST_LOG=info \
      nohup "$ABOT_BIN" --config "$CONFIG" > "$LOG_DIR/$name.log" 2>&1 &
      echo $! > "$PID_DIR/$name.pid"
      echo "  PID=$! -> $LOG_DIR/$name.log"
    done
    echo "All 8 bodies started."
    ;;
  stop)
    for name in "${BODIES[@]}"; do
      if [ -f "$PID_DIR/$name.pid" ]; then
        pid=$(cat "$PID_DIR/$name.pid")
        if kill -0 "$pid" 2>/dev/null; then
          echo "Stopping $name (PID $pid)"
          kill "$pid" 2>/dev/null || true
        else
          echo "$name not running (stale PID $pid)"
        fi
        rm -f "$PID_DIR/$name.pid"
      else
        echo "$name not running (no PID file)"
      fi
    done
    ;;
  status)
    for name in "${BODIES[@]}"; do
      if [ -f "$PID_DIR/$name.pid" ]; then
        pid=$(cat "$PID_DIR/$name.pid")
        if kill -0 "$pid" 2>/dev/null; then
          uptime=$(ps -o etime= -p "$pid" 2>/dev/null | tr -d ' ')
          echo "[ALIVE] $name (PID $pid, uptime: $uptime)"
        else
          echo "[DEAD]  $name (PID $pid exited)"
        fi
      else
        echo "[DEAD]  $name (no PID file)"
      fi
    done
    ;;
  logs)
    name="${2:-general-assistant}"
    tail -50 "$LOG_DIR/$name.log" 2>/dev/null || echo "No log for $name"
    ;;
  *)
    echo "Usage: $0 {start|stop|status|logs [name]}"
    ;;
esac
