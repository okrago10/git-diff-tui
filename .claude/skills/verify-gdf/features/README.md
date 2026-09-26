# gdf verification map

This directory is the maintained source for verifying the user-facing behavior of gdf (git-diff-tui). Read this index, then use the matching feature file as the recipe. All commands assume `H=.claude/skills/verify-gdf/gdf-verify.sh` run from the project root (see `../SKILL.md`).

## Baseline preconditions

- `$H build` has produced `target/release/gdf` and `$H doctor` reports `ok` for the binary.
- A fresh fixture per recipe: `R=$($H fixture <name>)`. Recipes that mutate the repo must not reuse another recipe's fixture.
- gdf started with `$H start <session> "$R"` and ready: `$H wait <session> "files ("`.
- The fixture list (top to bottom) is `● D old.txt`, `● A src/added.rs`, `● M src/lib.rs`, `○ M README.md`, `○ M long.rs`, `○ ? notes.md`, `○ M src/lib.rs`; status bar ` 7 files (3 staged, 4 unstaged)`.
- Never drive a gdf that this run did not start, and never start gdf in the user's checkout.

## Driving conventions

- Send input only with `$H keys` (tmux key names) or `tmux -L gdf-verify send-keys -l` for raw mouse sequences, then `$H settle`.
- Assert with `$H wait <session> "<text>"` or `$H screen <session> | grep`. Selection = the Files row starting with `▶ `.
- Pane is 120x30 unless a recipe says otherwise; the Diff pane shows 27 lines of diff and ~82 columns.
- Match substrings of the diff header (e.g. `b/long.rs`), not the whole first Diff line (see Gotchas in `../SKILL.md`).

## Proof and skip reporting

- Capture before and after each action: `$H capture <session> NN-<label>`. Artifacts land in `.verify-artifacts/<session>/` and survive `$H cleanup`.
- Colour/highlight claims need the `.ansi` capture, not the `.txt`.
- Repo-state claims (refresh, read-only) need `git -C "$R" status --porcelain` saved next to the capture.
- Record the feature ID and entry point (key name or mouse sequence) with every artifact.
- If an entry point cannot be reached, report the attempted command and the unmet precondition. Do not report it as verified via another key.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior, then exactly four H2 sections in this order: `Sub-features`, `How to get to it (user POV)`, `Driving it with gdf-verify`, `Gotchas`.

## Features

- [File list](./file-list.md) covers the Files pane: stage icons, change kinds, ordering, status-bar counts, empty repo.
- [File selection and diff preview](./file-selection.md) covers j/k/arrow navigation, bounds, and the highlighted diff shown for the selection.
- [Diff scrolling](./diff-scroll.md) covers vertical/horizontal scroll by keys and mouse wheel, jumps, and scroll reset on selection change.
- [Refresh](./refresh.md) covers `r` picking up repo changes made while gdf runs.
- [Startup and quit](./startup-quit.md) covers launching outside a repo, `q`/`Esc` exit, and the read-only guarantee.
