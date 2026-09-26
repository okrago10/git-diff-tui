#!/usr/bin/env bash
# Verification harness for gdf (git-diff-tui).
# Drives the real binary inside a private tmux server so runs never touch
# the user's own tmux sessions or their repository.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
BIN="$PROJECT_ROOT/target/${GDF_VERIFY_PROFILE:-release}/gdf"
SOCK="gdf-verify"                          # private tmux socket (tmux -L)
SCRATCH_ROOT="${GDF_VERIFY_SCRATCH:-${TMPDIR:-/tmp}/gdf-verify}"
EVIDENCE_ROOT="${GDF_VERIFY_EVIDENCE:-$PROJECT_ROOT/.verify-artifacts}"
COLS="${GDF_VERIFY_COLS:-120}"
ROWS="${GDF_VERIFY_ROWS:-30}"

# -f /dev/null: ignore the user's ~/.tmux.conf (default-shell etc.).
t() { tmux -f /dev/null -L "$SOCK" "$@"; }
# Exact-match target. A bare "-t s" prefix-matches, so it would hit "s1".
tgt() { printf '=%s:' "$1"; }
die() { echo "gdf-verify: $*" >&2; exit 1; }

usage() {
  cat <<EOF
usage: gdf-verify.sh <command> [args]

  build                         cargo build (release unless GDF_VERIFY_PROFILE=debug) -> $BIN
  fixture <name> [empty]        create disposable repo at $SCRATCH_ROOT/<name>, print its path
  start <session> <repo-dir>    launch gdf in a detached ${COLS}x${ROWS} tmux pane (cwd = repo-dir)
  wait <session> <text> [sec]   block until <text> appears on screen (default 10s)
  keys <session> <key>...       send tmux key names one by one (j, k, J, G, C-d, PageDown, Escape, ...), settling after each
  settle <session>              wait until the screen stops changing
  wheel <session> <dir>         mouse wheel up|down (3 lines) or left|right (4 cols) over the Diff pane
  screen <session>              print current screen (plain text)
  capture <session> <label>     save screen to evidence dir as <label>.txt and <label>.ansi
  alive <session>               exit 0 if gdf is still running in the pane
  doctor [session]              read-only health check
  stop <session>                send q, then kill the tmux session
  cleanup                       kill private tmux server and fixtures (evidence is kept)

evidence dir: $EVIDENCE_ROOT/<session>/
EOF
}

cmd_build() {
  local flag=()
  [[ "${GDF_VERIFY_PROFILE:-release}" == "release" ]] && flag=(--release)
  (cd "$PROJECT_ROOT" && cargo build --quiet "${flag[@]}")
  [[ -x "$BIN" ]] || die "build did not produce $BIN"
  echo "$BIN"
}

# Fixture layout (non-empty):
#   staged:    A src/added.rs, M src/lib.rs, D old.txt
#   unstaged:  M README.md, M src/lib.rs, M long.rs (60 lines changed + one 200-char line)
#   untracked: ? notes.md
cmd_fixture() {
  local name="${1:?fixture name}" mode="${2:-full}"
  local dir="$SCRATCH_ROOT/$name"
  rm -rf "$dir"; mkdir -p "$dir"
  cd "$dir"
  git init -q -b main
  git config user.email verify@example.invalid
  git config user.name gdf-verify
  git config commit.gpgsign false
  mkdir -p src
  printf '# fixture\n\nhello\n' > README.md
  printf 'pub fn one() -> u32 {\n    1\n}\n' > src/lib.rs
  printf 'to be deleted\n' > old.txt
  for i in $(seq 1 60); do echo "let v$i = $i;"; done > long.rs
  git add -A && git commit -qm init
  [[ "$mode" == "empty" ]] && { echo "$dir"; return; }

  printf 'pub fn added() -> bool {\n    true\n}\n' > src/added.rs
  printf 'pub fn one() -> u32 {\n    2\n}\n' > src/lib.rs
  git rm -q old.txt
  git add src/added.rs src/lib.rs
  printf 'pub fn one() -> u32 {\n    3 // unstaged\n}\n' > src/lib.rs
  printf '# fixture\n\nhello world\n' > README.md
  { for i in $(seq 1 60); do echo "let v$i = $((i * 10));"; done
    printf 'let wide = "%s";\n' "$(printf 'W%.0s' $(seq 1 200))"; } > long.rs
  printf 'untracked note\n' > notes.md
  echo "$dir"
}

cmd_start() {
  local s="${1:?session}" repo="${2:?repo dir}"
  [[ -x "$BIN" ]] || die "binary missing; run: $0 build"
  [[ -d "$repo" ]] || die "no such dir: $repo"
  t has-session -t "$(tgt "$s")" 2>/dev/null && die "session $s already exists (stop it or pick another name)"
  t new-session -d -s "$s" -x "$COLS" -y "$ROWS" -c "$repo" \
    "env TERM=xterm-256color $BIN; echo \"[gdf exited: \$?]\"; sleep 86400"
  t set-option -t "$(tgt "$s")" remain-on-exit on >/dev/null
  mkdir -p "$EVIDENCE_ROOT/$s"
  echo "started $s in $repo"
}

cmd_screen() { t capture-pane -p -t "$(tgt "${1:?session}")"; }

cmd_wait() {
  local s="${1:?session}" text="${2:?text}" secs="${3:-10}" i
  for ((i = 0; i < secs * 10; i++)); do
    cmd_screen "$s" | grep -qF -- "$text" && return 0
    sleep 0.1
  done
  echo "--- screen at timeout ---" >&2; cmd_screen "$s" >&2
  die "timed out after ${secs}s waiting for: $text"
}

cmd_keys() {
  local s="${1:?session}"; shift
  local k
  for k in "$@"; do t send-keys -t "$(tgt "$s")" "$k"; cmd_settle "$s"; done
}

# Mouse wheel via raw SGR sequences (gdf enables mouse capture). Column 60,
# row 10 is inside the Diff pane at the default 120x30 size.
cmd_wheel() {
  local s="${1:?session}" dir="${2:?up|down|left|right}" b
  case "$dir" in up) b=64 ;; down) b=65 ;; left) b=66 ;; right) b=67 ;;
    *) die "wheel direction must be up|down|left|right" ;; esac
  t send-keys -t "$(tgt "$s")" -l $'\e'"[<$b;60;10M"
  cmd_settle "$s"
}

# Wait until the screen is unchanged for 600ms (max ~8s). gdf redraws on a
# 100ms poll loop and highlighting a large diff blocks the redraw (debug
# build: ~0.9s for a 120-line diff), so a capture taken right after
# send-keys can show the previous frame. Settle is a heuristic: when a
# specific result is expected, `wait <text>` is the real assertion.
cmd_settle() {
  local s="${1:?session}" prev="" cur i
  sleep 0.2
  for ((i = 0; i < 14; i++)); do
    cur="$(cmd_screen "$s")"
    [[ "$cur" == "$prev" ]] && return 0
    prev="$cur"; sleep 0.6
  done
  echo "gdf-verify: warning: screen still changing after ~8s" >&2
}

cmd_capture() {
  local s="${1:?session}" label="${2:?label}" out="$EVIDENCE_ROOT/${1}"
  mkdir -p "$out"
  cmd_settle "$s"
  t capture-pane -p -t "$(tgt "$s")" > "$out/$label.txt"
  t capture-pane -p -e -t "$(tgt "$s")" > "$out/$label.ansi"
  echo "$out/$label.txt"
}

cmd_alive() {
  local s="${1:?session}"
  local pid
  pid="$(t display-message -p -t "$(tgt "$s")" '#{pane_pid}' 2>/dev/null)" || return 1
  pgrep -P "$pid" -f "^$BIN" >/dev/null
}

cmd_doctor() {
  local s="${1:-}" ok=1
  command -v tmux >/dev/null && echo "ok   tmux $(tmux -V)" || { echo "FAIL tmux not installed"; ok=0; }
  if [[ -x "$BIN" ]]; then
    local newest
    newest="$(find "$PROJECT_ROOT/src" "$PROJECT_ROOT/Cargo.toml" -newer "$BIN" -print -quit)"
    [[ -z "$newest" ]] && echo "ok   binary up to date: $BIN" \
      || { echo "FAIL binary older than $newest (run build)"; ok=0; }
  else echo "FAIL binary missing: $BIN (run build)"; ok=0; fi
  if [[ -n "$s" ]]; then
    if t has-session -t "$(tgt "$s")" 2>/dev/null; then
      echo "ok   session $s on private socket -L $SOCK (cwd $(t display-message -p -t "$(tgt "$s")" '#{pane_current_path}'))"
      cmd_alive "$s" && echo "ok   gdf running in pane" || { echo "FAIL gdf not running in pane (last line: $(cmd_screen "$s" | grep -v '^$' | tail -1))"; ok=0; }
    else echo "FAIL no session $s on -L $SOCK"; ok=0; fi
  fi
  echo "info evidence dir: $EVIDENCE_ROOT"
  ((ok)) || exit 1
}

cmd_stop() {
  local s="${1:?session}"
  t has-session -t "$(tgt "$s")" 2>/dev/null || { echo "no session $s"; return 0; }
  cmd_alive "$s" && t send-keys -t "$(tgt "$s")" q && sleep 0.3
  t kill-session -t "$(tgt "$s")"
  echo "stopped $s"
}

cmd_cleanup() {
  t kill-server 2>/dev/null || true
  rm -rf "$SCRATCH_ROOT"
  echo "cleaned tmux -L $SOCK and $SCRATCH_ROOT; evidence kept at $EVIDENCE_ROOT"
}

case "${1:-}" in
  build|fixture|start|wait|settle|wheel|keys|screen|capture|alive|doctor|stop|cleanup)
    c="$1"; shift; "cmd_$c" "$@" ;;
  *) usage; exit 2 ;;
esac
