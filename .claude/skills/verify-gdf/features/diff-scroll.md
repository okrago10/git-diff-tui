# Diff scrolling

The Diff pane scrolls vertically and horizontally for diffs larger than the pane, by keyboard or mouse wheel, and jumps to top/end/left edge.

## Sub-features

- `scroll-page` scrolls 10 lines with `J` / `PageDown` / `Ctrl+d` and back with `K` / `PageUp` / `Ctrl+u`.
- `scroll-jump` goes to the end with `G` and the top with `g`.
- `scroll-horizontal` scrolls columns right with `l` / `Right` and left with `h` / `Left`; `0` returns to column 0.
- `scroll-wheel` scrolls 3 lines per wheel notch vertically and 4 columns horizontally.
- `scroll-reset` resets both offsets when the selection changes.

## How to get to it (user POV)

- Select a file whose diff is longer or wider than the pane, then press the keys above or use the mouse wheel over the terminal.

## Driving it with gdf-verify

Preconditions:

- Fresh fixture `R=$($H fixture scr)` and session `scr` started and ready.
- `$H keys scr j j j j` then `$H wait scr "b/long.rs"`. `▶` is on `○ M long.rs`; Diff row 2 is `@@ -1,60 +1,61 @@`.

- **Before.** Run `$H capture scr 01-top`. Row 3 of the Diff pane is `-let v1 = 1;`.
- **Page down (J).** Run `$H keys scr J` and `$H capture scr 02-J`. Top Diff line is `-let v9 = 9;` (10 lines down from the header).
- **Page down aliases.** From `g`, `PageDown` and `C-d` each give the same screen as `02-J`. Capture `03-PageDown`, `04-C-d`.
- **Page up.** From `02-J`, `K`, `PageUp`, `C-u` each return to the `01-top` screen.
- **End (G).** Run `$H keys scr G` and `$H capture scr 05-G`. Last Diff row is the `+let wide = "WWW...` line; top Diff line is `+let v35 = 350;`.
- **Top (g).** Run `$H keys scr g`. Screen equals `01-top`.
- **Horizontal.** Run `$H keys scr l l` and `$H capture scr 06-l-l`. Diff row 1 begins `it a/long.rs` (header shifted left by the h-scroll step ×2). `$H keys scr h` shifts back one step; `$H keys scr 0` restores `01-top`. `Right` / `Left` behave as `l` / `h`.
- **Wheel.** From `g`, run `$H wheel scr down` and `$H capture scr 07-wheel-down`. Top Diff line is `-let v2 = 2;` (3 lines). `$H wheel scr right` shifts 4 columns (`-let v2` becomes ` v2 = 2;`). `up` / `left` reverse them.
- **Reset on selection.** After scrolling, `$H keys scr k` then `$H keys scr j`. `long.rs` shows `01-top` again.

## Gotchas

- Scroll only has an effect on `long.rs` in the fixture; other diffs fit in the pane, so a "no change" there is not a failure.
- `wheel` sends events at column 60, row 10 (inside the Diff pane at 120x30). Wheel events over the Files pane also scroll the diff (observed).
- With `GDF_VERIFY_PROFILE=debug`, selecting `long.rs` takes ~0.9s to redraw; always `wait "b/long.rs"` before scrolling.
- Changing `GDF_VERIFY_ROWS` changes how many lines `G` leaves visible; the expected top line for `G` above assumes 30 rows.
