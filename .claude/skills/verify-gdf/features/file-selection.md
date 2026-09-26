# File selection and diff preview

Moving the selection in the Files pane changes the Diff pane to show the syntax-highlighted diff of the selected entry; staged entries show index-vs-HEAD, unstaged show worktree-vs-index, untracked show the whole file as added.

## Sub-features

- `select-next` moves `▶` down one row with `j` or `Down`.
- `select-prev` moves `▶` up one row with `k` or `Up`.
- `select-bounds` stops at the first and last rows (no wrap).
- `preview-staged` shows the staged hunk (`src/lib.rs`: `-    1` / `+    2`).
- `preview-unstaged` shows the worktree hunk (`src/lib.rs`: `-    2` / `+    3 // unstaged`).
- `preview-untracked` shows `notes.md` content as added lines.
- `preview-highlight` colours added lines with a green background and syntax tokens in truecolor.

## How to get to it (user POV)

- Press `j` / `k` or the `Down` / `Up` arrow keys in gdf.

## Driving it with gdf-verify

Preconditions:

- Fresh fixture `R=$($H fixture sel)` and session `sel` started and ready. `▶` is on `● D old.txt`.

- **Before.** Run `$H capture sel 01-before`. The Diff pane contains `-to be deleted`.
- **Next with j.** Run `$H keys sel j` then `$H capture sel 02-after-j`. `▶` is on `● A src/added.rs`; Diff contains `+pub fn added() -> bool {`.
- **Next with Down.** Run `$H keys sel Down` then `$H wait sel "+    2"` and `$H capture sel 03-after-down`. `▶` is on `● M src/lib.rs`; Diff has `-    1` and `+    2`.
- **Unstaged twin.** Run `$H keys sel j j j j` then `$H wait sel "3 // unstaged"` and `$H capture sel 04-unstaged-lib`. `▶` is on `○ M src/lib.rs` (last row); Diff has `-    2` and `+    3 // unstaged`.
- **Lower bound.** Run `$H keys sel j` and `$H capture sel 05-past-end`. Screen is identical to `04`.
- **Prev with k / Up.** Run `$H keys sel k` and check `▶` on `○ ? notes.md` with Diff `+untracked note`; then `$H keys sel Up` and check `▶` on `○ M long.rs`. Capture each (`06-after-k`, `07-after-up`).
- **Upper bound.** Run `$H keys sel k k k k k k k` and `$H capture sel 08-top`. `▶` is on `● D old.txt`.
- **Highlight.** In `02-after-j.ansi`, `grep -c $'\e\[48;2;0;60;0m' .verify-artifacts/sel/02-after-j.ansi` is ≥ 1 (added-line background), and the line with `pub fn added` contains `38;2;` truecolor sequences.

## Gotchas

- `src/lib.rs` appears twice; identify the row by its icon, not the path.
- The first Diff line concatenates git header lines (`b/old.txtdeleted file mode ...`). Match substrings.
- An untracked file with no readable content shows `new file: <path>` instead of added lines.
- Changing selection resets both scroll offsets; see `diff-scroll.md`.
