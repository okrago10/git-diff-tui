# Startup and quit

gdf starts only inside a git repository (found by walking up from the cwd), exits cleanly with `q` or `Esc` restoring the terminal, and never modifies the repository.

## Sub-features

- `start-subdir` works when launched from a subdirectory of the repo.
- `start-norepo` prints an error and exits 1 outside any repository, without entering the TUI.
- `quit-q` exits 0 on `q`.
- `quit-esc` exits 0 on `Esc`.
- `read-only` leaves HEAD, index, and worktree unchanged after a full session.

## How to get to it (user POV)

- Run `gdf` in a terminal; press `q` or `Esc` to leave.

## Driving it with gdf-verify

Preconditions:

- Fresh fixture `R=$($H fixture sq)`.

- **Subdir start.** Run `$H start sq-sub "$R/src"` and `$H wait sq-sub "7 files ("`. Paths are still repo-relative (`src/lib.rs`). Capture `01-subdir`.
- **Quit with q.** Run `$H keys sq-sub q` and `$H wait sq-sub "[gdf exited: 0]"`, then `$H capture sq-sub 02-after-q` and `$H alive sq-sub; echo $?` prints `1`. The capture shows the shell's normal screen (alt-screen left), not the TUI frame.
- **Quit with Esc.** `$H start sq-esc "$R"`, `$H wait sq-esc "files ("`, `$H keys sq-esc Escape`, `$H wait sq-esc "[gdf exited: 0]"`, capture `01-after-esc`.
- **No repo.** Run `mkdir -p /tmp/gdf-verify/norepo`, `$H start sq-norepo /tmp/gdf-verify/norepo`, `$H wait sq-norepo "[gdf exited: 1]"`, capture `01-norepo`. Screen shows `Error: could not find repository at '.'` and `Run this command inside a git repository.`
- **Read-only.** Before starting any session, save `git -C "$R" rev-parse HEAD; git -C "$R" status --porcelain; git -C "$R" ls-files -s | sha1sum` to `.verify-artifacts/sq-sub/00-repo-before.txt`. After driving (select, scroll, refresh, quit), save the same to `03-repo-after.txt`. `diff` of the two is empty.

## Gotchas

- `/tmp/gdf-verify/norepo` must not be inside any git repository, otherwise gdf discovers the parent repo. Check with `git -C /tmp/gdf-verify/norepo rev-parse 2>&1` (must fail).
- `Escape` via tmux can be delayed by tmux's `escape-time`; `wait` for the exit line instead of sleeping.
- The pane stays open after gdf exits (`sleep` placeholder) so the exit code is readable; `$H stop` still removes it.
