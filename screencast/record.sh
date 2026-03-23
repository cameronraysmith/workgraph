#!/usr/bin/env bash
# record.sh — Record a hero screencast scenario with asciinema.
# Usage: ./record.sh <scenario>
#   scenario: heist | haiku | pancakes
#
# This script:
# 1. Resets the demo project to a clean state
# 2. Starts asciinema recording with idle-time compression
# 3. Launches wg tui inside the recording
#
# The human operator types the scenario prompt into the TUI coordinator chat,
# waits for tasks to complete, then exits (Ctrl-C TUI, Ctrl-D recording).

set -euo pipefail

SCENARIO="${1:-}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="${WG_DEMO_DIR:-/tmp/wg-hero-demo}"
CAST_DIR="$SCRIPT_DIR/recordings"

if [ -z "$SCENARIO" ]; then
    echo "Usage: $0 <scenario>"
    echo "  heist    — Plan a Heist Movie Night"
    echo "  haiku    — Write a Haiku Pipeline"
    echo "  pancakes — Debug a Pancake Recipe"
    exit 1
fi

# Validate scenario
case "$SCENARIO" in
    heist)
        PROMPT="Plan a heist movie night for the team — snacks, movie picks, and a debate."
        ;;
    haiku)
        PROMPT="Write three haiku about Rust programming, then pick the best one."
        ;;
    pancakes)
        PROMPT="My pancakes are flat. Diagnose the problem and fix my recipe."
        ;;
    *)
        echo "Unknown scenario: $SCENARIO"
        echo "Choose: heist | haiku | pancakes"
        exit 1
        ;;
esac

# Ensure demo dir exists
if [ ! -d "$DEMO_DIR/.workgraph" ]; then
    echo "Demo project not found at $DEMO_DIR. Run setup-demo.sh first."
    exit 1
fi

# Reset the project for a clean recording
cd "$DEMO_DIR"
echo "Resetting demo project..."
wg service stop 2>/dev/null || true
rm -rf .workgraph/graph.jsonl .workgraph/service/ .workgraph/output/
wg init 2>/dev/null || true
wg config --max-agents 4
wg config --model sonnet
wg config --coordinator-executor claude

# Start service so TUI shows healthy status badge
echo "Starting service..."
wg service start --force 2>/dev/null || wg service start 2>/dev/null
sleep 5

# Wait for coordinator agent to be ready (up to 60s)
echo "Waiting for coordinator agent..."
READY=0
for i in $(seq 1 12); do
    if wg service status 2>/dev/null | grep -q "Uptime:"; then
        READY=1; break
    fi
    sleep 5
done
[ $READY -eq 0 ] && echo "WARNING: Coordinator may not be ready yet"
wg service status 2>&1 | head -5

# Create recordings directory
mkdir -p "$CAST_DIR"

TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
CAST_FILE="$CAST_DIR/${SCENARIO}-${TIMESTAMP}.cast"

echo ""
echo "=== Recording: $SCENARIO ==="
echo "Output: $CAST_FILE"
echo ""
echo "Prompt to type in TUI coordinator chat:"
echo "  $PROMPT"
echo ""
echo "Workflow:"
echo "  1. TUI will open automatically (service is running)"
echo "  2. Type (or paste) the prompt above into the coordinator chat"
echo "  3. Watch agents work — tasks will appear and complete in the graph"
echo "  4. When all tasks are done, press Ctrl-C to exit TUI"
echo "  5. Press Ctrl-D to stop the asciinema recording"
echo ""
echo "Starting recording in 3 seconds..."
sleep 3

# Record with idle time compression (2s max gap)
asciinema rec --idle-time-limit 2 --command "wg tui" "$CAST_FILE"

# Stop the service after recording
wg service stop 2>/dev/null || true

echo ""
echo "Recording saved: $CAST_FILE"
echo "Preview: asciinema play $CAST_FILE"
echo "Upload: asciinema upload $CAST_FILE"
