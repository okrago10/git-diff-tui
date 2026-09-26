# Refresh

Pressing `r` re-reads the repository so files changed, added, or reverted while gdf is running appear in the list, with the selection clamped to the new list.

## Sub-features

- `refresh-add` shows a newly created untracked file.
- `refresh-remove` drops entries whose changes were reverted or deleted.
- `refresh-clamp` keeps the selected row index, clamped to the new last row.
- `refresh-counts` updates the status-bar counts.

## How to get to it (user POV)

- Press `r` in gdf after changing files in another terminal.

## Driving it with gdf-verify

Preconditions:

- Fresh fixture `R=$($H fixture ref)` and session `ref` started and ready.

- **Before.** Run `$H capture ref 01-before` and `git -C "$R" status --porcelain > .verify-artifacts/ref/01-porcelain.txt`.
- **Add.** Run `echo new > "$R/fresh.txt"`, then `$H capture ref 02-no-refresh-yet` (list unchanged: gdf does not watch files), then `$H keys ref r` and `$H wait ref "fresh.txt"`, `$H capture ref 03-after-add`. Row `○ ? fresh.txt` appears between `README.md` and `long.rs`; status bar ` 8 files (3 staged, 5 unstaged)`.
- **Remove + clamp.** Run `$H keys ref G j j j j j j j` so `▶` is on the last row. Then `git -C "$R" checkout -q -- src/lib.rs README.md long.rs && rm "$R/notes.md" "$R/fresh.txt"`, `$H keys ref r`, `$H wait ref "3 files ("`, `$H capture ref 04-after-remove`. Only the 3 staged rows remain and `▶` is on `● M src/lib.rs`.
- **Repo fact.** `git -C "$R" status --porcelain > .verify-artifacts/ref/04-porcelain.txt` lists exactly `D  old.txt`, `A  src/added.rs`, `M  src/lib.rs`.

## Gotchas

- There is no auto-refresh; `02-no-refresh-yet` proves that and must match `01-before`.
- Selection is kept by row index, not by path: after adding `fresh.txt`, a selection on row 5 moves from `long.rs` to `fresh.txt`.
- Runtime error state (`Error: listing changed files: ...` in Diff, ` ERROR ` in status bar) has no known real-path trigger: libgit2 keeps the index cached, so corrupting or replacing `.git/index` during a run still refreshes normally. It is covered by unit tests in `src/ui.rs`. Report it as not reachable via the UI rather than faking it.
