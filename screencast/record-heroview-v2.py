#!/usr/bin/env python3
"""Record heroview v2 screencast: season haiku demo.

Follows the storyboard from docs/design/heroview-screencast-v2-script.md.
Six phases:
0. CLI Orient: wg service start, wg status (fast typing)
1. Launch TUI: wg tui (no screen clear)
2. Chat Prompt: type + submit "Write haiku about the four seasons"
3. Graph Growth: 6 tasks appear one by one (0.8s stagger)
4. Task Progression: parallel execution, staggered completion
5. Results Reveal: haiku output lingers on screen
6. Survey + Exit: final graph review, clean exit

All task creation and progression is injected (deterministic).
Output: screencast/recordings/heroview-v2-raw.cast
"""

import json
import os
import random
import subprocess
import sys
import time

random.seed(42)

# Import recording harness
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib
record_harness = importlib.import_module("record-harness")
RecordingHarness = record_harness.RecordingHarness
_verify_cast = record_harness._verify_cast

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CAST_FILE = os.path.join(SCRIPT_DIR, "recordings", "heroview-v2-raw.cast")
DEMO_DIR = f"/tmp/wg-heroview-v2-{os.getpid()}"

PROMPT = "Write haiku about the four seasons"

CLAUDE_MD = """\
# Haiku Seasons Demo

When the user asks to write haiku about the seasons, decompose into these tasks:

1. spring-haiku — Write a haiku about spring (no dependencies)
2. summer-haiku — Write a haiku about summer (no dependencies)
3. autumn-haiku — Write a haiku about autumn (no dependencies)
4. winter-haiku — Write a haiku about winter (no dependencies)
5. compile-collection — Gather all four haiku (after spring-haiku, summer-haiku, autumn-haiku, winter-haiku)
6. format-output — Final formatting (after compile-collection)

Use exactly these task IDs. Create all 6 tasks using wg add with --after dependencies.
Tasks 1-4 MUST be parallel (no dependencies). Keep your response brief.
Do NOT create any other tasks or subtasks.
"""

# Task definitions for injection
TASKS = [
    ("Spring haiku", "spring-haiku", None, "Write a haiku about spring"),
    ("Summer haiku", "summer-haiku", None, "Write a haiku about summer"),
    ("Autumn haiku", "autumn-haiku", None, "Write a haiku about autumn"),
    ("Winter haiku", "winter-haiku", None, "Write a haiku about winter"),
    ("Compile collection", "compile-collection",
     "spring-haiku,summer-haiku,autumn-haiku,winter-haiku",
     "Gather all four haiku into a formatted collection"),
    ("Format output", "format-output", "compile-collection",
     "Final formatting and presentation"),
]

CHAT_RESPONSE = (
    "I'll create a haiku for each season and compile them:\n\n"
    "1. **spring-haiku** \u2014 spring poem\n"
    "2. **summer-haiku** \u2014 summer poem\n"
    "3. **autumn-haiku** \u2014 autumn poem\n"
    "4. **winter-haiku** \u2014 winter poem\n"
    "5. **compile-collection** \u2014 gather all four\n"
    "6. **format-output** \u2014 final presentation\n\n"
    "Tasks 1\u20134 run in parallel. Creating now..."
)

HAIKU = {
    "spring-haiku": "Cherry blossoms fall\nSoft rain wakes the sleeping earth\nNew leaves reach for light",
    "summer-haiku": "Heat shimmers on stone\nCicadas drone through long days\nThunder breaks the calm",
    "autumn-haiku": "Crimson leaves descend\nCold wind strips the maple bare\nGeese trace southern lines",
    "winter-haiku": "Snow blankets the field\nBare branches etch gray silence\nBreath crystallizes",
}

COLLECTION_LINES = [
    "Four Seasons \u2014 A Haiku Collection",
    "Spring: Cherry blossoms fall / Soft rain wakes the sleeping earth / New leaves reach for light",
    "Summer: Heat shimmers on stone / Cicadas drone through long days / Thunder breaks the calm",
    "Autumn: Crimson leaves descend / Cold wind strips the maple bare / Geese trace southern lines",
    "Winter: Snow blankets the field / Bare branches etch gray silence / Breath crystallizes",
]

_start_time = None


def log(msg):
    elapsed = time.monotonic() - _start_time if _start_time else 0
    print(f"[{elapsed:7.1f}s] {msg}", file=sys.stderr)


def wg(*args):
    """Run wg command in the demo directory."""
    try:
        return subprocess.run(
            ["wg"] + list(args),
            capture_output=True, text=True,
            cwd=DEMO_DIR, timeout=30,
        )
    except subprocess.TimeoutExpired:
        return None


def setup_demo():
    """Initialize a fresh demo project."""
    if os.path.exists(DEMO_DIR):
        subprocess.run(["rm", "-rf", DEMO_DIR])
    os.makedirs(DEMO_DIR)

    subprocess.run(["git", "init", "-q"], cwd=DEMO_DIR, check=True)
    subprocess.run(
        ["git", "commit", "--allow-empty", "-m", "init", "-q"],
        cwd=DEMO_DIR, check=True,
    )

    wg("init")

    # Write CLAUDE.md
    with open(os.path.join(DEMO_DIR, "CLAUDE.md"), "w") as f:
        f.write(CLAUDE_MD)

    # Configure: no auto-agent-spawning, no coordinator (we drive everything)
    wg("config", "--max-agents", "0")

    config_path = os.path.join(DEMO_DIR, ".workgraph", "config.toml")
    with open(config_path) as f:
        config = f.read()

    # Disable coordinator agent so no .coordinator-0 / .compact-0 are created
    config = config.replace(
        "coordinator_agent = true", "coordinator_agent = false"
    )
    # Hide system tasks from graph/TUI display
    config = config.replace(
        "show_system_tasks = true", "show_system_tasks = false"
    )
    # Also hide running system tasks
    config = config.replace(
        "show_running_system_tasks = false", "show_running_system_tasks = false"
    )

    with open(config_path, "w") as f:
        f.write(config)

    log(f"Demo project at {DEMO_DIR}")


def inject_chat_history():
    """Write chat history so TUI shows the conversation."""
    chat = [
        {
            "role": "user",
            "text": PROMPT,
            "timestamp": "2026-03-25T12:00:01+00:00",
            "edited": False,
        },
        {
            "role": "assistant",
            "text": CHAT_RESPONSE,
            "timestamp": "2026-03-25T12:00:05+00:00",
            "edited": False,
        },
    ]
    chat_file = os.path.join(DEMO_DIR, ".workgraph", "chat-history.json")
    with open(chat_file, "w") as f:
        json.dump(chat, f)


def inject_tasks_staggered():
    """Create tasks with stagger for visible graph growth."""
    for title, tid, after, desc in TASKS:
        cmd = ["add", title, "--id", tid, "-d", desc]
        if after:
            cmd.extend(["--after", after])
        wg(*cmd)
        time.sleep(0.8)


def inject_haiku_and_complete():
    """Inject haiku log entries and drive task progression."""
    # Claim all 4 haiku tasks (parallel)
    for tid in ["spring-haiku", "summer-haiku", "autumn-haiku", "winter-haiku"]:
        wg("claim", tid)
    time.sleep(0.3)

    # Complete haiku tasks with staggered timing, injecting output
    for tid, haiku in HAIKU.items():
        for line in haiku.split("\n"):
            wg("log", tid, line)
        wg("done", tid)
        time.sleep(0.5)

    # compile-collection
    wg("claim", "compile-collection")
    time.sleep(0.3)
    for line in COLLECTION_LINES:
        wg("log", "compile-collection", line)
    wg("done", "compile-collection")
    time.sleep(0.3)

    # format-output
    wg("claim", "format-output")
    time.sleep(0.2)
    wg("log", "format-output", "Collection formatted and ready")
    wg("done", "format-output")


# ── Phases ─────────────────────────────────────────────────

def phase_0_cli(h):
    """Phase 0: CLI Orient — wg service start, wg status."""
    log("=== Phase 0: CLI Orient ===")

    h.wait_for("$", timeout=5)

    # wg service start
    h.type_naturally("wg service start", wpm=200)
    h.send_keys("Enter")
    log("Sent: wg service start")
    h.sleep(2)

    # wg status
    h.type_naturally("wg status", wpm=200)
    h.send_keys("Enter")
    log("Sent: wg status")
    h.sleep(2)
    h.flush_frame()

    snap = h.snapshot()
    log(f"Phase 0 done. Has output: {len(snap.strip()) > 20}")


def phase_1_launch(h):
    """Phase 1: Launch TUI — no screen clear, type wg tui after previous output."""
    log("=== Phase 1: Launch TUI ===")

    # NO clear — just type after previous output
    h.type_naturally("wg tui --recording", wpm=200)
    h.send_keys("Enter")
    log("Sent: wg tui")

    # Wait for TUI to render
    found = h.wait_for("Chat", timeout=15, interval=0.5)
    if found:
        log("TUI rendered")
    else:
        log("WARNING: TUI render not detected, trying alternative check")
        found = h.wait_for("Graph", timeout=5, interval=0.5)

    # Let viewer orient to TUI layout
    h.sleep(2)
    h.flush_frame()

    # System tasks should be hidden via config (show_system_tasks = false)
    # Do NOT press '.' here — it would TOGGLE them ON.

    # Shrink inspector panel to make graph prominent
    for _ in range(3):
        h.send_keys("I")
        h.sleep(0.4)
    h.flush_frame()
    log("Shrunk inspector (Shift+I x3)")

    h.sleep(1)
    return found


def phase_2_chat(h):
    """Phase 2: Chat Prompt — type and submit the haiku request."""
    log("=== Phase 2: Chat Prompt ===")

    # Enter chat input
    h.send_keys("c")
    h.sleep(0.8)
    h.flush_frame()

    # Type prompt fast
    h.type_naturally(PROMPT, wpm=200)
    h.sleep(0.3)
    h.flush_frame()

    # Submit
    h.send_keys("Enter")
    log(f"Submitted: '{PROMPT}'")
    h.flush_frame()

    # Brief pause for "coordinator thinking"
    h.sleep(1.5)

    # Inject chat history so TUI shows the response
    inject_chat_history()
    h.sleep(1)
    h.flush_frame()
    log("Chat response injected")


def phase_3_graph_growth(h):
    """Phase 3: Graph Growth — tasks appear one by one.

    Uses harness sleeps (not time.sleep) between task injections so the
    recording captures each task appearing incrementally.
    """
    log("=== Phase 3: Graph Growth ===")

    # Inject tasks one at a time with harness sleeps so TUI captures
    # each task appearing individually.
    for i, (title, tid, after, desc) in enumerate(TASKS):
        cmd = ["add", title, "--id", tid, "-d", desc]
        if after:
            cmd.extend(["--after", after])
        wg(*cmd)
        log(f"  Injected {tid}")
        h.sleep(0.8)
        h.flush_frame()

    log("All 6 tasks injected")

    # Let TUI fully refresh
    h.sleep(1)
    h.flush_frame()

    snap = h.snapshot()
    has_tasks = any(tid in snap for tid in ["spring-haiku", "compile-collection"])
    log(f"Tasks visible in TUI: {has_tasks}")


def phase_4_progression(h):
    """Phase 4: Task Progression — parallel execution, staggered completion."""
    log("=== Phase 4: Task Progression ===")

    # Exit chat mode to focus on graph
    h.send_keys("Escape")
    h.sleep(0.5)

    # Switch to Detail tab to show task metadata during progression
    h.send_keys("1")
    h.sleep(0.5)

    # Claim all 4 haiku tasks simultaneously — they go in-progress
    for tid in ["spring-haiku", "summer-haiku", "autumn-haiku", "winter-haiku"]:
        wg("claim", tid)
    h.sleep(1.5)
    h.flush_frame()
    log("4 haiku tasks claimed (in-progress)")

    # Navigate down through tasks to show parallel execution
    # Graph order: spring(0), compile(1), format(2), summer(3), autumn(4), winter(5)
    for _ in range(3):
        h.send_keys("Down")
        h.sleep(0.8)
    h.flush_frame()

    # Complete haiku tasks with stagger + inject output
    for tid, haiku in HAIKU.items():
        for line in haiku.split("\n"):
            wg("log", tid, line)
        wg("done", tid)
        log(f"  {tid}: done")
        h.sleep(0.8)
        h.flush_frame()

    h.sleep(1)

    # compile-collection auto-ready → claim → inject → done
    wg("claim", "compile-collection")
    h.sleep(1)
    for line in COLLECTION_LINES:
        wg("log", "compile-collection", line)
    wg("done", "compile-collection")
    log("  compile-collection: done")
    h.sleep(0.8)
    h.flush_frame()

    # format-output
    wg("claim", "format-output")
    h.sleep(0.5)
    wg("log", "format-output", "Collection formatted and ready")
    wg("done", "format-output")
    log("  format-output: done")
    h.sleep(1)
    h.flush_frame()

    log("All tasks complete")


def phase_5_results(h):
    """Phase 5: Results Reveal — show haiku output on screen.

    Navigate to spring-haiku, switch to Log tab, let viewer read the poem.
    Then navigate to compile-collection for the full collection.
    """
    log("=== Phase 5: Results Reveal ===")

    # After phase_4's Escape from chat, we're in Normal mode but with
    # RightPanel focus (Up/Down scroll the panel, not navigate tasks).
    # Press Escape once to return focus to the Graph panel.
    h.send_keys("Escape")
    h.sleep(0.3)

    # Navigate to top of graph (may be system tasks at top)
    for _ in range(15):
        h.send_keys("Up")
        h.sleep(0.1)
    h.sleep(0.5)

    # Switch to Detail tab — shows "◉ Task Title" for the selected task
    h.send_keys("1")
    h.sleep(0.5)
    h.flush_frame()

    # Navigate down until the Detail panel header shows "Spring haiku"
    # (not a system task like "Coordinator 0" or "Compact 0").
    # The panel header format is: "◉ Task Title [+]"
    found_spring = False
    for i in range(10):
        snap = h.snapshot()
        # Check detail panel header for "Spring haiku" (the task's TITLE, not ID)
        if "Spring haiku" in snap or "spring-haiku" in snap.split("│")[-1] if "│" in snap else False:
            # More robust: check the right side of a border separator
            for line in snap.split("\n"):
                # The detail panel is after the │ border character
                if "│" in line:
                    right = line.split("│", 1)[-1]
                    if "Spring" in right and "haiku" in right.lower():
                        found_spring = True
                        break
            if found_spring:
                log(f"Selected spring-haiku (after {i} Down presses)")
                break
        h.send_keys("Down")
        h.sleep(0.3)

    if not found_spring:
        # Fallback: navigate a fixed number of positions past system tasks.
        # System tasks are at positions 0-1 (.coordinator-0, .compact-0).
        # spring-haiku should be at position 2. Go back to top and try.
        log("WARNING: Could not confirm spring-haiku in detail panel, using fallback")
        for _ in range(15):
            h.send_keys("Up")
            h.sleep(0.1)
        h.sleep(0.3)
        # Down×2 to skip 2 system tasks
        h.send_keys("Down")
        h.sleep(0.3)
        h.send_keys("Down")
        h.sleep(0.3)
        h.flush_frame()

    # Switch to Log tab (2) to show haiku content
    h.send_keys("2")
    h.sleep(1)
    h.flush_frame()

    snap = h.snapshot()
    has_haiku = any(kw in snap for kw in ["Cherry", "blossoms", "Soft rain", "leaves"])
    log(f"spring-haiku Log tab — haiku visible: {has_haiku}")

    # LINGER — 5+ seconds for viewer to read the haiku
    h.sleep(5)
    h.flush_frame()
    log("Haiku displayed for 5s")

    # Navigate down one to compile-collection for full collection view
    h.send_keys("Down")
    h.sleep(1)
    h.flush_frame()

    snap = h.snapshot()
    has_collection = any(kw in snap for kw in ["Four Seasons", "Collection", "Spring:"])
    log(f"compile-collection visible: {has_collection}")

    # Brief linger on collection
    h.sleep(2)
    h.flush_frame()
    log("Collection view displayed")


def phase_6_exit(h):
    """Phase 6: Final Survey + Exit."""
    log("=== Phase 6: Survey + Exit ===")

    # Navigate to top
    for _ in range(10):
        h.send_keys("Up")
        h.sleep(0.15)
    h.sleep(0.5)

    # Switch to Detail tab (1) for clean view
    h.send_keys("1")
    h.sleep(0.5)

    # Slow scroll through completed graph — all tasks should be green/done
    for _ in range(5):
        h.send_keys("Down")
        h.sleep(1.0)
        h.flush_frame()

    # Final pause — viewer absorbs the completed graph
    h.sleep(2)
    h.flush_frame()
    log("Final graph survey done")

    # Exit TUI
    h.send_keys("q")
    h.sleep(1.5)
    h.flush_frame()

    # Hold on shell prompt for clean ending
    h.sleep(1)
    h.flush_frame()
    log("Clean exit")


# ── Main ────────────────────────────────────────────────────

def record():
    global _start_time
    _start_time = time.monotonic()

    log("=== Heroview v2 Screencast Recording ===")
    log(f"Cast file: {CAST_FILE}")

    # Setup
    log("=== Setup ===")
    setup_demo()

    # Do NOT pre-start the service — let Phase 0's `wg service start` do it
    # so the viewer sees a clean "Service started" message.

    try:
        shell_cmd = (
            f"cd {DEMO_DIR} && "
            f"export PS1='\\[\\033[1;32m\\]$ \\[\\033[0m\\]' && "
            f"exec bash --norc --noprofile"
        )

        with RecordingHarness(
            cast_file=CAST_FILE,
            cwd=DEMO_DIR,
            shell_command=shell_cmd,
            idle_time_limit=5.0,
        ) as h:
            phase_0_cli(h)
            tui_ok = phase_1_launch(h)
            if not tui_ok:
                log("ERROR: TUI did not load. Aborting.")
                return False

            phase_2_chat(h)
            phase_3_graph_growth(h)
            phase_4_progression(h)
            phase_5_results(h)
            phase_6_exit(h)

            duration = h.duration
            frames = h.frame_count

        # Summary
        log(f"\n{'=' * 60}")
        log(f"Recording complete: {duration:.1f}s ({duration/60:.1f} min), {frames} frames")
        log(f"Cast file: {CAST_FILE}")
        log(f"{'=' * 60}")

        # Verify
        log("Verifying cast file...")
        ok = _verify_cast(CAST_FILE)
        return ok

    finally:
        wg("service", "stop")
        log(f"Demo dir: {DEMO_DIR}")


if __name__ == "__main__":
    success = record()
    sys.exit(0 if success else 1)
