# File list

The Files pane lists every changed file in the repo gdf was started in, marking each with a stage icon and a change-kind letter, and the status bar counts them.

## Sub-features

- `list-icons` shows `●` for staged and `○` for unstaged/untracked entries.
- `list-kinds` shows `D` delete, `A` add, `M` modify, `?` untracked (also `R` rename, `C` copy, `T` typechange).
- `list-order` lists all staged entries first, then unstaged, each group sorted by path.
- `list-dual` shows a file changed in both index and worktree twice (once `●`, once `○`).
- `list-counts` shows ` N files (S staged, U unstaged)` in the status bar; untracked counts as unstaged.
- `list-empty` shows `No changes detected` and `0 files (0 staged, 0 unstaged)` for a clean repo.

## How to get to it (user POV)

- Run `gdf` inside a git repository. The list is shown immediately; there is no command to open it.

## Driving it with gdf-verify

Preconditions:

- Fresh fixture `R=$($H fixture list)` and session `list` started and ready.

- **Initial list.** Launch and capture. Run `$H capture list 01-initial`. Rows 2-8 of the capture read, in order, `▶ ● D old.txt`, `  ● A src/added.rs`, `  ● M src/lib.rs`, `  ○ M README.md`, `  ○ M long.rs`, `  ○ ? notes.md`, `  ○ M src/lib.rs` (inside `│...│`).
- **Counts.** Run `$H screen list | tail -1`. It starts with ` 7 files (3 staged, 4 unstaged)`.
- **Repo fact.** Run `git -C "$R" status --porcelain > .verify-artifacts/list/01-porcelain.txt`. It lists `D  old.txt`, `A  src/added.rs`, `MM src/lib.rs`, ` M README.md`, ` M long.rs`, `?? notes.md`, matching the icons above (`MM` = `list-dual`).
- **Empty repo.** `$H stop list`, then `E=$($H fixture list-empty empty)`, `$H start list-empty "$E"`, `$H wait list-empty "No changes detected"`, `$H capture list-empty 01-empty`. Status bar reads ` 0 files (0 staged, 0 unstaged)`.

## Gotchas

- Sorting is byte-wise: `README.md` sorts before `long.rs` because uppercase precedes lowercase.
- A staged `git mv` does not show as `R`: rename detection is off, so `git -C "$R" mv README.md RENAMED.md` shows `● D README.md` and `● A RENAMED.md` (observed). `R`, `C`, `T` are therefore not reachable with the fixture; report them as unreached rather than verified.
- File names are sanitised: control characters render as `�`. A path containing an escape sequence must not recolour the screen.
