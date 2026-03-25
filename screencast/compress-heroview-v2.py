#!/usr/bin/env python3
"""Compress heroview-v2-raw.cast into a 30-50s time-lapse.

Scene-aware compression per the v2 storyboard:
- Phase 0 (CLI): fast typing (3x), short output waits
- Phase 1 (Launch): fast typing, moderate TUI render
- Phase 2 (Chat): fast typing, brief coordinator pause
- Phase 3 (Graph Growth): SLOW — viewer must see each task appear (0.8s stagger)
- Phase 4 (Progression): moderate — keep status transitions visible
- Phase 5 (Results): SLOW — 5s linger on haiku output
- Phase 6 (Survey): moderate navigation, clean exit

Critical: Phase 5 results linger must survive compression.
"""

import json
import os
import re
import sys

RAW_PATH = "screencast/recordings/heroview-v2-raw.cast"
COMPRESSED_PATH = "screencast/recordings/heroview-v2-compressed.cast"
TIMEMAP_PATH = "screencast/recordings/heroview-v2-timemap.json"

# Per-scene: (interaction_speed, activity_speed, short_wait_cap, long_wait_cap)
# Tuned to hit ~35-40s compressed from ~60s raw.
SCENE_PARAMS = {
    "cli":          (2.5, 1.8, 0.30, 0.20),
    "launch":       (2.5, 1.8, 0.35, 0.25),
    "chat":         (2.5, 1.3, 0.40, 0.30),
    "graph_growth": (1.0, 1.0, 0.70, 0.60),
    "progression":  (1.2, 1.2, 0.40, 0.35),
    "results":      (1.0, 1.0, 0.90, 0.80),
    "survey":       (1.3, 1.5, 0.30, 0.25),
}

DEFAULT_PARAMS = (1.5, 2.0, 0.20, 0.15)


def load_cast(path):
    with open(path, "r") as f:
        lines = f.readlines()
    header = json.loads(lines[0])
    frames = [json.loads(line) for line in lines[1:]]
    return header, frames, lines


def detect_scenes(frames):
    """Detect scene boundaries from content patterns.

    Uses a combination of content matching and minimum-duration guards
    to prevent adjacent scene transitions.
    """
    scenes = []
    current_scene = "cli"
    scenes.append(("cli", 0))

    def last_scene_time():
        idx = scenes[-1][1]
        return frames[idx][0] if idx < len(frames) else 0

    for i, frame in enumerate(frames):
        data = frame[2] if len(frame) >= 3 else ""
        # Strip ANSI escapes for pattern matching
        plain = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', data)
        t = frame[0]

        # Detect TUI launch — "Chat" or "Graph" panel headers appear
        if current_scene == "cli" and ("Chat" in plain or "Graph" in plain):
            scenes.append(("launch", i))
            current_scene = "launch"

        # Detect chat input — user typed "haiku" (after >=2s in launch)
        elif current_scene == "launch" and "haiku" in plain.lower() and t - last_scene_time() > 2:
            scenes.append(("chat", i))
            current_scene = "chat"

        # Detect graph growth — "spring" appears in graph (after >=2s in chat)
        elif current_scene == "chat" and "spring" in plain.lower() and t - last_scene_time() > 2:
            scenes.append(("graph_growth", i))
            current_scene = "graph_growth"

        # Detect progression — "in-progress" visible (after >=3s in graph_growth)
        elif current_scene == "graph_growth" and "in-progress" in plain.lower() and t - last_scene_time() > 3:
            scenes.append(("progression", i))
            current_scene = "progression"

        # Detect results — haiku content visible (after >=5s in progression)
        elif current_scene == "progression" and t - last_scene_time() > 5 and (
            "Cherry blossoms" in plain or "Four Seasons" in plain or
            "cherry blossoms" in plain.lower() or "four seasons" in plain.lower()
        ):
            scenes.append(("results", i))
            current_scene = "results"

        # Detect survey — after >=5s in results
        elif current_scene == "results" and t - last_scene_time() > 5:
            scenes.append(("survey", i))
            current_scene = "survey"

    return scenes


def get_scene_for_frame(scenes, frame_idx):
    """Return the scene name for a given frame index."""
    scene_name = scenes[0][0]
    for name, start_idx in scenes:
        if frame_idx >= start_idx:
            scene_name = name
    return scene_name


def compress(frames, scenes):
    """Compute compressed timestamps."""
    if not frames:
        return [], []

    compressed_time = frames[0][0]
    new_timestamps = [compressed_time]
    timemap = [{"compressed_s": round(compressed_time, 6), "real_s": round(frames[0][0], 6)}]

    for i in range(1, len(frames)):
        real_gap = frames[i][0] - frames[i - 1][0]
        scene = get_scene_for_frame(scenes, i)
        params = SCENE_PARAMS.get(scene, DEFAULT_PARAMS)
        interaction_speed, activity_speed, short_wait_cap, long_wait_cap = params

        if real_gap > 3.0:
            compressed_gap = long_wait_cap
        elif real_gap > 1.0:
            compressed_gap = short_wait_cap
        elif real_gap > 0.3:
            compressed_gap = real_gap / activity_speed
        else:
            compressed_gap = real_gap / interaction_speed

        compressed_time += compressed_gap
        new_timestamps.append(compressed_time)
        timemap.append({
            "compressed_s": round(compressed_time, 6),
            "real_s": round(frames[i][0], 6),
        })

    return new_timestamps, timemap


def write_compressed(header, frames, new_timestamps, raw_lines, out_path):
    with open(out_path, "w", newline="") as f:
        header_line = raw_lines[0]
        f.write(header_line)
        if not header_line.endswith("\n"):
            f.write("\n")

        for i, frame in enumerate(frames):
            new_frame = [round(new_timestamps[i], 6), frame[1], frame[2]]
            f.write(json.dumps(new_frame, ensure_ascii=False) + "\n")


def main():
    print(f"Loading {RAW_PATH}...")
    header, frames, raw_lines = load_cast(RAW_PATH)

    print(f"  Header: {header['width']}x{header['height']}, {len(frames)} frames")
    raw_duration = frames[-1][0] - frames[0][0]
    print(f"  Raw duration: {raw_duration:.1f}s ({raw_duration/60:.1f} min)")

    print("Detecting scenes...")
    scenes = detect_scenes(frames)
    for name, idx in scenes:
        t = frames[idx][0]
        print(f"  {name}: frame {idx} ({t:.1f}s)")

    print("Compressing...")
    new_timestamps, timemap = compress(frames, scenes)

    compressed_duration = new_timestamps[-1] - new_timestamps[0]
    ratio = raw_duration / compressed_duration if compressed_duration > 0 else 1
    print(f"  Compressed duration: {compressed_duration:.1f}s")
    print(f"  Compression ratio: {ratio:.1f}x")

    if compressed_duration < 25 or compressed_duration > 60:
        print(f"  WARNING: Duration {compressed_duration:.1f}s outside 25-60s target")

    print(f"Writing {COMPRESSED_PATH}...")
    write_compressed(header, frames, new_timestamps, raw_lines, COMPRESSED_PATH)

    print(f"Writing {TIMEMAP_PATH}...")
    with open(TIMEMAP_PATH, "w") as f:
        json.dump(timemap, f, indent=2)

    # Validation
    print("\nValidation:")
    with open(COMPRESSED_PATH) as f:
        comp_header = json.loads(f.readline())
    assert comp_header["width"] == 65, f"Width: {comp_header['width']}"
    assert comp_header["height"] == 38, f"Height: {comp_header['height']}"
    print(f"  Dimensions: {comp_header['width']}x{comp_header['height']} OK")

    with open(COMPRESSED_PATH) as f:
        comp_lines = f.readlines()
    comp_frames = [json.loads(line) for line in comp_lines[1:]]
    max_gap = max(
        comp_frames[i][0] - comp_frames[i-1][0]
        for i in range(1, len(comp_frames))
    )
    print(f"  Max gap: {max_gap:.2f}s")

    assert len(comp_frames) == len(frames), "Frame count mismatch"
    print(f"  Frames: {len(comp_frames)} OK")

    for i in range(len(frames)):
        assert comp_frames[i][2] == frames[i][2], f"Content differs at frame {i}"
    print(f"  Content: identical OK")

    crlf_count = sum(1 for f in comp_frames if "\r\n" in f[2])
    print(f"  CR+LF frames: {crlf_count}/{len(comp_frames)}")

    print(f"\nDone! {raw_duration:.1f}s -> {compressed_duration:.1f}s ({ratio:.1f}x)")


if __name__ == "__main__":
    main()
