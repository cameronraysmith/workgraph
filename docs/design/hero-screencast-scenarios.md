# Hero Screencast Scenarios

Three scenarios for the website hero screencast showing a real TUI coordinator workflow.

## Scenario 1: "Plan a Heist Movie Night"

**Human types:** `Plan a heist movie night for the team — snacks, movie picks, and a debate.`

**Task graph:**
```
research-heist-movies  ─┐
                        ├─► pick-final-movie
research-snack-pairings ─┘        │
                                  ▼
                           send-invitation
```

- `research-heist-movies` — brainstorm 3 heist films (parallel)
- `research-snack-pairings` — suggest snacks that match the genre (parallel)
- `pick-final-movie` — synthesize picks into one choice (after both)
- `send-invitation` — draft the invite (after pick)

**Why it's fun:** Two agents visibly race to research movies vs snacks simultaneously. The graph lights up with parallel work, then converges. Relatable, low-stakes, charming.

## Scenario 2: "Write a Haiku Pipeline"

**Human types:** `Write three haiku about Rust programming, then pick the best one.`

**Task graph:**
```
haiku-borrow-checker ──┐
haiku-cargo-build   ───┼─► judge-haiku
haiku-unsafe-block  ──┘
```

- `haiku-borrow-checker` / `haiku-cargo-build` / `haiku-unsafe-block` — each writes one haiku (all parallel)
- `judge-haiku` — picks the winner (after all three)

**Why it's fun:** Three agents sprint in parallel on a silly creative task. The fan-out/fan-in pattern is immediately legible. Developers chuckle at Rust-themed poetry.

## Scenario 3: "Debug a Pancake Recipe"

**Human types:** `My pancakes are flat. Diagnose the problem and fix my recipe.`

**Task graph:**
```
diagnose-flatness ─► fix-recipe ─► taste-test
                     fix-presentation ──┘
```

- `diagnose-flatness` — identify the issue (first)
- `fix-recipe` + `fix-presentation` — adjust ingredients and plating (parallel, after diagnosis)
- `taste-test` — final verdict (after both fixes)

**Why it's fun:** A "debugging" metaphor applied to cooking. Shows the pipeline pattern clearly: diagnose → parallel fixes → integration. The task names read like a real engineering workflow, which is the point.

## Time Compression

- **Record with `asciinema`** (already supported: `wg tui` has asciinema compat).
- **Skip idle time:** Use `asciinema rec --idle-time-limit 2` to cap gaps at 2 seconds. Agent think-time collapses; state transitions stay visible.
- **Show only transitions:** The TUI updates the graph view live. Each status change (open → in-progress → done) is a visible frame. No need to show agent output — the graph tells the story.
- **Target length:** 30–45 seconds after compression. Human types one sentence, graph fills in, agents race, tasks complete, done.
