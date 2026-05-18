#!/bin/bash
# Start/stop/check scsynth from WSL2
# Usage: hum-server start|stop|check [port]

PORT=${2:-57110}
GATEWAY=$(ip route show default | awk '{print $3}')
SCSYNTH_PS1="$(dirname "$0")/start-scsynth.ps1"

case "${1:-check}" in
  start)
    echo "Starting scsynth on Windows (port $PORT)..."
    powershell.exe -ExecutionPolicy Bypass -File "$(wslpath -w "$SCSYNTH_PS1")" -Port "$PORT" 2>/dev/null
    sleep 2
    # Verify
    if echo -ne '/status\x00,\x00\x00\x00' | nc -u -w2 "$GATEWAY" "$PORT" | xxd | grep -q "status.reply"; then
      echo "scsynth alive at $GATEWAY:$PORT"
      echo "export SCSYNTH_HOST=$GATEWAY:$PORT"
    else
      echo "ERROR: scsynth not responding at $GATEWAY:$PORT"
      echo "Check Windows firewall: UDP port $PORT must be allowed"
      exit 1
    fi
    ;;
  stop)
    echo "Stopping scsynth..."
    powershell.exe -Command "Get-Process scsynth -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null
    echo "Stopped"
    ;;
  check)
    if echo -ne '/status\x00,\x00\x00\x00' | nc -u -w2 "$GATEWAY" "$PORT" 2>/dev/null | xxd | grep -q "status.reply"; then
      echo "scsynth alive at $GATEWAY:$PORT"
    else
      echo "scsynth not responding at $GATEWAY:$PORT"
      echo "Run: hum-server start"
      exit 1
    fi
    ;;
  *)
    echo "Usage: hum-server start|stop|check [port]"
    exit 1
    ;;
esac
