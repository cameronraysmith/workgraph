# Screencast: Coordinator Interaction Flow

*Design for the "hero" screencast demonstrating the full human-coordinator workflow.*

**Produced by:** screencast-design-interaction  
**Date:** 2026-03-24  
**Status:** Design complete, ready for implementation  
**Downstream:** screencast-implement-interaction

---

## Motivation

The existing showcase screencast (record-showcase.py) demonstrates the TUI but buries the most compelling feature: **talking to the coordinator and watching it orchestrate work**. The coordinator interaction appears in Scene 5 of 6, after minutes of watching pre-created tasks progress. By that point, the viewer has already seen the graph but hasn't seen the thing that makes wg different — *the human says one sentence and the system decomposes it into a coordinated multi-agent workflow*.

The new screencast leads with interaction. The coordinator conversation is the opening act, not the encore.

---

## Script Overview

**Target length:** 50–60 seconds (compressed). Raw recording ~3–8 minutes.  
**Terminal:** 65×38 (matches existing harness).  
**Scenario:** Haiku News — user asks coordinator to build a haiku pipeline, watches agents work, inspects live output, then asks for a feature addition.

### Scene Map

| # | Scene | Compressed Time | What Viewer Learns |
|---|-------|-----------------|-------------------|
| 1 | Launch + Orient | 3–5s | TUI layout: graph left, inspector right, chat tab visible |
| 2 | Talk to Coordinator | 5–8s | Typing a message, coordinator responds with a plan |
| 3 | Tasks Appear + Agents Spawn | 8–12s | Graph fills in, parallel execution starts automatically |
| 4 | Live Detail View | 10–15s | **Key moment:** watching agent output in real time |
| 5 | Conversation Round 2 | 5–8s | User asks for a change, coordinator adapts the graph |
| 6 | Final Survey + Exit | 5–8s | All tasks done, clean graph, exit |

**Total: ~45–55 seconds compressed.**

---

## Detailed Storyboard

### Scene 1: Launch + Orient (0s – 4s compressed)

**What the viewer sees:**
- Shell prompt → `wg tui` typed naturally
- TUI renders: graph panel (left) shows coordinator node or empty graph. Chat tab (right) is active with empty conversation. Service status badge shows "Running" in status bar.

**User actions:**
1. Type `wg tui` (natural speed, ~40 WPM)
2. Press Enter
3. Wait for TUI to render (~1s)

**Teaching purpose:** "This is a terminal UI. Left = task graph, right = chat with the coordinator. The service is already running."

**Compression:** Cut startup to ~2s. Keep the typing at real speed.

**Pre-requisites:** The service must already be running (`wg service start` done before recording begins or as part of setup). The graph should be empty or have only the coordinator node — the viewer needs to see tasks appear from nothing.

---

### Scene 2: Talk to Coordinator (4s – 12s compressed)

**What the viewer sees:**
- Chat input activates (cursor appears in input area)
- User types: `Build a haiku news pipeline — scrape headlines, generate haiku for each, and publish an API`
- User presses Enter
- Coordinator response streams in: describes the decomposition plan, mentions task names
- Tasks begin appearing in the graph panel as the coordinator runs `wg add`

**User actions:**
1. Press `c` to enter chat input mode
2. Type the prompt naturally (~50 WPM)
3. Press Enter to submit
4. Watch coordinator response stream in

**Teaching purpose:** "You talk to the coordinator in natural language. It decomposes your request into a task graph — you don't manually create tasks."

**Compression:** Typing is real-speed (~5s). Coordinator think-time (30–90s real) compresses to ~3s. The key visual event is tasks appearing in the graph as the coordinator creates them.

**Implementation notes:**
- The coordinator must be configured to accept creative/demo tasks (CLAUDE.md patch as in existing setup-demo.sh)
- Fallback: If coordinator doesn't respond within 3 minutes, inject pre-built tasks and chat history (as in record-showcase.py's `inject_fallback_tasks()`)
- The prompt should produce 5–8 tasks with clear dependency structure (pipeline + fan-out)

**Expected task graph:**
```
scrape-headlines ──┐
                   ├──► wire-haiku-engine ──► draft-haikus ──► review-quality
analyze-mood ──────┘                                                │
                                                                    ▼
count-syllables ──────────────────────────────────────────► publish-api
build-pun-db ─────────────────────────────────────────────┘
```

---

### Scene 3: Tasks Appear + Agents Spawn (12s – 22s compressed)

**What the viewer sees:**
- Tasks materialize in the graph panel as coordinator creates them
- Edges form showing dependency relationships
- First tasks flip from `open` → `in-progress` (color change)
- Agent count indicator in status bar shows active agents
- Multiple tasks show active/spinner indicators simultaneously

**User actions:**
1. Press `Esc` to exit chat input (return to graph focus)
2. Press `↓` a few times to navigate through tasks
3. (Optional) Press `t` to toggle edge tracing — shows magenta upstream / cyan downstream edges

**Teaching purpose:** "The service automatically dispatches agents to ready tasks. No human intervention. Multiple agents work in parallel."

**Compression:** Heavy. Real agent spawn time (30–60s) compresses to ~8s. Key visual events are the status transitions: open → in-progress for each task.

**Implementation notes:**
- Use `h.wait_for("in-progress", timeout=120)` to detect first agent spawn
- Navigate slowly enough that the viewer can follow: 1–1.5s per arrow key press
- Edge tracing is visually impressive but optional — skip if timing is tight

---

### Scene 4: Live Detail View (22s – 37s compressed)

**This is the most important scene.** The viewer sees agents producing output in real time.

**What the viewer sees:**

**Sub-scene 4a: Detail Tab (5s)**
- User selects an in-progress task (e.g., `draft-haikus`)
- Presses `1` to switch to Detail tab
- Detail tab shows task metadata: status, description, assigned agent
- If agent is actively working, a "last written Xs ago" footer appears and content updates live
- User scrolls down (if needed) to see the output section

**Sub-scene 4b: Log Tab (5s)**
- User presses `2` to switch to Log tab
- Shows reverse-chronological activity log with timestamps
- Agent log entries appear showing work progress (e.g., "Generating haiku for headline #3...")
- Content grows as agent writes new log entries

**Sub-scene 4c: Firehose Tab — The Money Shot (5–8s)**
- User presses `8` to switch to Firehose tab
- **Merged stream from ALL active agents simultaneously**
- Each line is color-coded by agent (different agents = different colors)
- Lines appear in real time as agents write output
- Auto-scroll follows new content (tail mode)
- The viewer sees 2–3 agents writing interleaved output — this is the "wow" moment

**User actions:**
1. Select an in-progress task with arrow keys
2. Press `1` (Detail tab) — pause 2–3s
3. Press `2` (Log tab) — pause 3s, scroll if needed
4. Press `8` (Firehose tab) — pause 4–5s to let the viewer absorb
5. Press `0` (back to Chat tab) or `Esc` (back to graph)

**Teaching purpose:** "You can inspect any task's live output. The Firehose tab shows ALL agents at once — it's a live activity stream of your entire workforce."

**Compression:** Moderate. This scene should play at near-real speed for the Firehose portion — the whole point is watching live output appear. Brief pauses (2–3s per tab) let the viewer read. Idle gaps between output bursts compress normally.

**Implementation notes:**
- The Firehose tab auto-tails by default — no user action needed to follow new content
- The Detail tab refreshes on output.log mtime changes (via `last_detail_output_mtime`)
- For best visual impact, time this scene when multiple agents are actively working (e.g., after fan-out tasks become ready)
- If agents have already finished by this point, the Log tab will still have rich content to show

### Feasibility Assessment: Capturing Live Detail View Output

**Approach 1: Real-time capture (RECOMMENDED)**

The recording harness (`record-harness.py`) captures tmux frames at a configurable FPS (5–15). Since the TUI live-refreshes the Detail/Log/Firehose tabs via filesystem watcher + polling (1s interval), real-time capture works:

- **Detail tab:** Refreshes on `output.log` mtime change. The TUI polls every ~1s. With FPS=5–10, we capture every visual update. ✅
- **Log tab:** Refreshes on `graph.jsonl` change (log entries stored there). Same polling. ✅
- **Firehose tab:** Refreshes on `update_firehose()` which reads from `agents/<id>/output.log`. Updates on every slow-path tick (1s) or fast-path filesystem event. ✅

**Timing challenge:** The agent needs to be actively producing output when we switch to the tab. Solution: check task status before switching tabs — only switch to Firehose when `wg agents --alive` shows ≥2 working agents.

**Tradeoff:** Real agent work is non-deterministic. Some recordings may catch great output moments, others may catch idle gaps. Mitigation: the compressor removes idle gaps, and we can retry the recording if the first attempt misses the sweet spot.

**Approach 2: Injected output (FALLBACK)**

If real-time capture proves unreliable, pre-inject log entries (as `record-showcase.py` does with `inject_haiku_logs()`):

1. After tasks start, use `wg log <task> "..."` to inject rich output
2. Switch to Detail/Log tabs — injected content appears immediately
3. For Firehose: write directly to `agents/<id>/output.log` to simulate agent output

**Tradeoff:** Deterministic timing but loses authenticity. The "live" feel is fake. Use only if real agents consistently miss the capture window.

**Approach 3: Hybrid (BEST OF BOTH)**

Run real agents AND inject supplementary log entries during slow periods:

1. Start real agents normally
2. While waiting for agent output, inject a few sample log entries to ensure the tabs have content
3. The real agent output will also appear, creating a richer stream
4. Time tab switches to coincide with real agent activity (detected via `wg agents --alive`)

This gives deterministic baseline content plus authentic live output.

**Recommendation:** Start with Approach 1 (real-time). If the first 2 recording attempts don't capture good Firehose content, switch to Approach 3 (hybrid). Avoid Approach 2 unless deadline pressure requires it.

---

### Scene 5: Conversation Round 2 (37s – 44s compressed)

**What the viewer sees:**
- User switches to Chat tab (press `0`)
- Types a follow-up: `Headlines are boring. Add a roast mode.`
- Coordinator responds, creates new tasks
- Graph expands with new tasks (3 roast-mode tasks)
- New tasks immediately start getting dispatched

**User actions:**
1. Press `0` (Chat tab)
2. Press `c` (chat input)
3. Type follow-up naturally
4. Press Enter
5. Watch response + new tasks appear
6. Press `Esc` to return to graph

**Teaching purpose:** "The graph is living. You can talk to the coordinator at any point to adjust the plan. New tasks slot into the existing dependency graph."

**Compression:** Same as Scene 2 — typing real-speed, coordinator think-time compressed. If coordinator doesn't respond, inject fallback tasks + chat history (same pattern as existing showcase).

**Implementation notes:**
- Reuse the existing `inject_fallback_tasks()` and `inject_fallback_chat()` from record-showcase.py
- The roast-mode tasks should depend on existing tasks (e.g., `build-snark-filter` after `count-syllables`)
- This scene demonstrates that wg is iterative, not one-shot

---

### Scene 6: Final Survey + Exit (44s – 52s compressed)

**What the viewer sees:**
- Navigate through completed tasks
- Graph shows mix of done tasks (initial wave) and in-progress/done tasks (second wave)
- Brief pause on a completed roast task's Log tab to show snarky haiku content
- Press `q` to exit TUI
- Shell prompt returns

**User actions:**
1. Navigate to a roast task (arrow keys)
2. Press `2` (Log tab) — pause to show content
3. Press `q` to quit

**Teaching purpose:** "Every task's output is preserved and inspectable. The full workflow — from one sentence to coordinated multi-agent results — happens in the terminal."

**Compression:** Navigation at real speed. Exit is quick.

---

## Complete Keystroke Sequence

```
1.  wg tui                                    [launch]
2.  (wait for TUI render)
3.  c                                         [enter chat mode]
4.  Build a haiku news pipeline — scrape       [type naturally]
    headlines, generate haiku for each,
    and publish an API
5.  Enter                                     [submit]
6.  (wait for coordinator response + tasks)
7.  Esc                                       [return to graph focus]
8.  ↓ ↓ ↓                                     [navigate to in-progress task]
9.  (wait for agents to start)
10. 1                                         [Detail tab]
11. (pause 2-3s — observe live refresh)
12. 2                                         [Log tab]
13. (pause 3s — observe log entries)
14. 8                                         [Firehose tab]
15. (pause 4-5s — THE money shot)
16. 0                                         [Chat tab]
17. c                                         [chat input]
18. Headlines are boring. Add a roast mode.   [type naturally]
19. Enter                                     [submit]
20. (wait for coordinator + new tasks)
21. Esc                                       [return to graph]
22. ↓ ↓ ... (navigate to roast task)          [find new tasks]
23. 2                                         [Log tab — show roast content]
24. (pause 3s)
25. q                                         [quit]
```

---

## Time Compression Summary

| Scene | Real Time | Compressed | Ratio | Method |
|-------|-----------|-----------|-------|--------|
| 1. Launch | 3–5s | 4s | ~1:1 | Trim startup |
| 2. Coordinator Chat | 40–120s | 8s | ~10:1 | Compress think-time |
| 3. Agents Spawn | 30–120s | 10s | ~10:1 | Compress wait, keep transitions |
| 4. Detail View | 30–60s | 15s | ~3:1 | Moderate — keep live output visible |
| 5. Round 2 | 40–120s | 7s | ~12:1 | Same as Scene 2 |
| 6. Survey + Exit | 10–20s | 6s | ~2:1 | Navigation real-speed |
| **Total** | **~3–8 min** | **~50s** | **~5:1** | — |

**Compression rules:**
- User keystrokes: real speed (the viewer should be able to follow what the human does)
- Coordinator think-time: compress heavily (30–90s → 2–3s)
- Agent work-in-progress: moderate compression for Scene 4 (live output is the payoff), heavy for other scenes
- Tab content pauses: 2–5s real (enough to read a few lines)
- Idle gaps: cap at 0.3s

---

## Technical Blockers and Solutions

### Blocker 1: Coordinator Response Latency

**Problem:** Coordinator LLM calls take 30–120s. If the coordinator is slow or fails, the recording stalls.

**Solution:** Three-tier fallback (same pattern as existing showcase):
1. **Primary:** Wait up to 3 minutes for real coordinator response
2. **Retry:** If no response, send a simpler follow-up message
3. **Fallback:** Inject pre-built tasks and chat history using `inject_fallback_tasks()` + `inject_fallback_chat()`

**Risk:** Low. The existing showcase already handles this successfully.

### Blocker 2: Agents Finish Before Scene 4

**Problem:** If agents are fast (e.g., Haiku model), all tasks may complete before we switch to the Firehose tab, resulting in no live output to show.

**Solution:**
- Use enough tasks (6–8) that some are still running by Scene 4
- Monitor task status with `wg agents --alive` before switching tabs
- If agents are done, switch to Log/Detail tabs instead of Firehose (still good content, just not live)
- Consider using Sonnet model (slower than Haiku) for demo tasks to give more working time
- Hybrid approach: inject supplementary log entries if needed

**Risk:** Medium. Timing depends on model speed and API latency. Multiple recording attempts may be needed.

### Blocker 3: Firehose Tab May Be Empty

**Problem:** If no agents are actively writing to `output.log` at the moment we switch to Firehose, the tab shows "No active agents producing output."

**Solution:**
- Check `wg agents --alive` count before switching to Firehose
- Only switch to Firehose when ≥2 agents are working
- If zero agents working, skip Firehose and show Log tab with historical content instead
- Inject sample output to agents' output.log files as backup

**Risk:** Medium. Same timing dependency as Blocker 2.

### Blocker 4: tmux Capture Misses Fast Updates

**Problem:** The recording harness polls tmux at FPS=5–15. If the TUI renders a brief flash (e.g., status transition), the poll may miss it.

**Solution:**
- Use FPS=10+ for Scene 4 (live output scenes)
- The TUI holds state between refreshes — as long as we capture within the 1s refresh interval, we get the content
- This is not a real blocker — the harness has been proven reliable at FPS=5 in the existing showcase

**Risk:** Low.

### Blocker 5: Terminal Size for Readability

**Problem:** 65×38 is compact. Long agent output lines may wrap awkwardly in the Firehose tab.

**Solution:**
- 65×38 is the established size for the recording harness and works well for the graph panel
- The Firehose tab truncates long lines rather than wrapping (it shows one-line-per-output-event)
- If readability is poor, consider increasing to 80×38 or 100×40 (requires harness config change)
- Test with a quick trial recording before committing to the full session

**Risk:** Low. Existing recordings at 65×38 are readable.

---

## Setup Script Modifications

The existing `setup-demo.sh` needs minor modifications:

1. **CLAUDE.md patch:** Already includes "Accept ALL task types" — no change needed
2. **Model config:** Use `sonnet` for coordinator (fast enough responses, creates good task graphs)
3. **Max agents:** Set to 3–4 for visible parallelism without overwhelming the graph
4. **Service pre-start:** Start the service BEFORE the recording begins, so Scene 1 shows TUI launch directly (no service start typing needed)

---

## Differences from Existing Showcase

| Aspect | Current Showcase (record-showcase.py) | New Design |
|--------|--------------------------------------|------------|
| Opening | `wg viz` → `wg service start` → `wg tui` | `wg tui` (service pre-started) |
| Coordinator chat | Scene 5 of 6 (afterthought) | Scene 2 of 6 (the lead) |
| Live output | Scene 3 shows Detail/Log/Output tabs | Scene 4 adds **Firehose** tab as the hero moment |
| Second interaction | Single "roast mode" addition | Same, but positioned as natural continuation |
| Graph starts | Pre-populated with 8 tasks | **Empty** — tasks appear from coordinator chat |
| Story arc | "Watch agents work on pre-made tasks" | "Talk to coordinator → watch it orchestrate → inspect live output → iterate" |

The fundamental shift: the current showcase shows wg as a **task execution system**. The new design shows wg as a **conversational orchestration system**.

---

## Implementation Checklist for screencast-implement-interaction

1. [ ] Modify `setup-demo.sh` to pre-start service (or create new setup variant)
2. [ ] Create `record-interaction.py` based on `record-showcase.py` structure
3. [ ] Implement Scene 1–6 with timing checks and fallbacks
4. [ ] Add Firehose tab navigation (key `8`) — verify it works in recording
5. [ ] Test with 2–3 recording attempts to validate timing
6. [ ] Create `compress-interaction.py` with scene-specific compression params
7. [ ] Verify compressed output is 45–60 seconds
8. [ ] Verify no NaN time displays or blank lines (depends on screencast-fix-nan, screencast-fix-blank-lines)

---

*End of design document. Artifact of task screencast-design-interaction.*
