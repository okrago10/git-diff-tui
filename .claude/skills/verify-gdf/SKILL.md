---
name: verify-gdf
description: Launch and drive the real gdf (git-diff-tui) terminal UI in a private tmux session against a disposable git repo, send keystrokes/mouse-wheel input, and capture screen evidence. Use when you need to prove a change to gdf's file list, diff preview, scrolling, refresh, highlighting, or startup/quit behavior works in the actual TUI, not just in cargo tests.
---

# verify-gdf

`gdf` is a read-only ratatui TUI: left pane `Files` (changed files), right pane `Diff` (highlighted diff of the selected file), bottom status bar. It reads the git repo found by walking up from its **cwd**; there are no CLI flags or env vars. It never writes to the repo.

Everything goes through `gdf-verify.sh` (next to this file). It uses a private tmux socket (`tmux -L gdf-verify`) without loading `~/.tmux.conf`, and targets sessions by exact name (a bare `-t s` would prefix-match `s1`), so it cannot touch the user's tmux sessions, and it runs gdf only inside disposable fixture repos under `/tmp/gdf-verify/`.

```bash
H=.claude/skills/verify-gdf/gdf-verify.sh   # run from repo root
$H                                          # prints usage
```

Feature recipes live in [`features/README.md`](features/README.md). Read the matching feature file before driving.

## Launch

gdf is short-lived: build once, then one tmux session per drive.

```bash
$H build                       # cargo build --release -> target/release/gdf
R=$($H fixture f1)             # disposable repo, prints its path
$H start s1 "$R"               # detached 120x30 pane, cwd = $R
$H wait s1 "files ("           # ready: status bar " 7 files (3 staged, 4 unstaged)" rendered
```

- Default profile is **release**. The debug build takes ~0.9s to highlight the 120-line `long.rs` diff, which makes captures race. Use `GDF_VERIFY_PROFILE=debug` only if you must, and then rely on `wait <text>`, not `settle`.
- Pane size: `GDF_VERIFY_COLS` / `GDF_VERIFY_ROWS` (default 120x30). Set before `start`.
- `fixture <name> empty` creates a repo with a commit and no changes.
- The fixture (non-empty) produces exactly this list, in this order:

  ```
  ▶ ● D old.txt          (staged delete)
    ● A src/added.rs     (staged add)
    ● M src/lib.rs       (staged modify: 1 -> 2)
    ○ M README.md        (unstaged)
    ○ M long.rs          (unstaged: 60 lines rewritten + one 200-char "let wide" line => scroll target)
    ○ ? notes.md         (untracked)
    ○ M src/lib.rs       (unstaged modify: 2 -> 3 // unstaged)
  ```

Teardown of one drive: `$H stop s1` (sends `q`, then kills the session).

## Doctor

Run first, and again whenever output looks wrong:

```bash
$H doctor s1      # or `$H doctor` without a session before launching
```

It checks: tmux present; binary exists and is newer than `src/` and `Cargo.toml` (else `FAIL ... run build`); the session exists on the private socket and shows its cwd; gdf is actually a child of the pane (not exited). Non-zero exit = do not drive; fix what it names. If gdf died, `$H screen s1` shows its stderr and `[gdf exited: N]`.

## Drive

```bash
$H keys s1 j j j j        # tmux key names, sent one at a time, screen settled after each
$H keys s1 J G g l 0      # J/K PageDown/PageUp C-d C-u g G h l 0 r q Escape Down Up Left Right
$H wait s1 "b/long.rs"    # assert text appears (default 10s timeout; prints screen on failure)
$H screen s1              # plain-text screen to stdout
```

- Assert with `wait <text>` / `grep` on `screen`, not on timing. `keys` settles (screen unchanged 600ms) but that is a heuristic.
- Stable handles: pane titles ` Files ` / ` Diff `; selection marker `▶ ` at the start of a Files row; stage icons `●` (staged) `○` (unstaged); kind letters `M A D R C T ?`; status bar ` N files (S staged, U unstaged)`; hints `j/k: select  J/K: scroll  h/l: h-scroll  r: refresh  q: quit`; empty state `No changes detected`; error state `Error: ...` in Diff pane and ` ERROR ` in the status bar.
- Mouse wheel (gdf enables SGR mouse capture): `$H wheel s1 down|up|left|right` sends one notch (3 lines / 4 columns) as a raw SGR sequence at column 60, row 10 of the Diff pane, then settles.

- To change repo state mid-run (e.g. for refresh), edit files inside `$R` with normal shell commands, then press `r`.
- Isolation: each drive gets its own session name and ideally its own fixture name; any number can run side by side. Never `start` in the user's checkout or `$PROJECT_ROOT`: gdf is read-only, but its screen would reflect the user's uncommitted work and the proof would not be reproducible.

## Evidence

```bash
$H capture s1 01-initial      # -> .verify-artifacts/s1/01-initial.txt (plain) + .ansi (with colors)
```

- Location: `.verify-artifacts/<session>/` in the project root (gitignored), override with `GDF_VERIFY_EVIDENCE`. Cleanup never deletes it.
- Capture **before and after** each action with numbered labels (`01-before-j`, `02-after-j`), so the proof shows the transition, not just the end screen.
- `.txt` is for assertions (selection, text, scroll offset). `.ansi` is for highlighting/colour proof: `cat -v` shows e.g. `^[[48;2;0;60;0m+` (green background on added lines) and `^[[38;2;...m` truecolor syntax tokens. View it with `cat file.ansi` in a terminal.
- Proof standards:
  - Drive gdf through keystrokes/mouse in the real binary. Do not call `App` methods or rely on `cargo test` as the proof; unit tests are separate evidence.
  - Pair the visible state with the underlying repo fact: e.g. after a refresh proof, also record `git -C "$R" status --porcelain` next to the capture.
  - gdf promises read-only: when a change could touch that, record `git -C "$R" status --porcelain` and `git -C "$R" rev-parse HEAD` before and after the run and show they are identical.
  - Clean exit proof: after `q`/`Escape`, `$H screen s1` shows `[gdf exited: 0]` and `$H alive s1` exits 1.

## Cleanup

```bash
$H stop s1        # per session
$H cleanup        # kills the private tmux server (-L gdf-verify) and rm -rf /tmp/gdf-verify; keeps .verify-artifacts/
```

`cleanup` only touches the private socket and the scratch dir it created. Never `pkill gdf` / `tmux kill-server` without `-L gdf-verify`. Run `cleanup` after failed iterations too. Afterwards, `ls .verify-artifacts/<session>/` must still list your captures.

## Known display quirk (as of this skill's creation)

The first Diff line concatenates the git file header without separators, e.g. `diff --git a/old.txt b/old.txtdeleted file mode 100644index ...`. It is current behaviour, not a harness artifact; match on substrings like `b/long.rs` rather than the whole header line.
