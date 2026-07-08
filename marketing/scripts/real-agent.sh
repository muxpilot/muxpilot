#!/usr/bin/env bash
# real-agent.sh — capture MuxPilot showing GENUINE live coding-agent state.
#
# Spins up a detached tmux session with a couple of panes each running a cheap
# real coding agent against OpenRouter, waits for them to start working, then
# records the REAL muxpilot picker (which reads live tmux state) with VHS.
#
# Gated on OPENROUTER_API_KEY — skips gracefully if unset. Uses a cheap model so
# a capture costs pennies. Tears the session down on exit.
#
#   OPENROUTER_API_KEY=sk-or-... ./scripts/real-agent.sh
#
# Env knobs:
#   OPENROUTER_MODEL   default: qwen/qwen-2.5-coder-7b-instruct (cheap)
#   AGENT_PANES        default: 2
#   AGENT_CLI          force one of: opencode | aider | crush (else auto-detect)
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/paths.sh"

if [ -d "$HOME/.local/share/mise/shims" ]; then
  PATH="$HOME/.local/share/mise/shims:$PATH"; export PATH
fi

MODEL="${OPENROUTER_MODEL:-qwen/qwen-2.5-coder-7b-instruct}"
PANES="${AGENT_PANES:-2}"
SESSION="muxpilot-live-demo"

# ── gates ───────────────────────────────────────────────────────────────────
if [ -z "${OPENROUTER_API_KEY:-}" ]; then
  cat >&2 <<'EOF'
real-agent: OPENROUTER_API_KEY is not set — skipping live capture.

  Get a key at https://openrouter.ai/keys, then:
    export OPENROUTER_API_KEY=sk-or-...
    make -C marketing real
EOF
  exit 0
fi

for tool in tmux vhs; do
  command -v "$tool" >/dev/null 2>&1 || { echo "real-agent: missing $tool" >&2; exit 1; }
done

# ── detect a coding-agent CLI that speaks OpenRouter ────────────────────────
detect_cli() {
  if [ -n "${AGENT_CLI:-}" ]; then echo "$AGENT_CLI"; return; fi
  for c in opencode aider crush; do
    command -v "$c" >/dev/null 2>&1 && { echo "$c"; return; }
  done
  echo ""
}
CLI="$(detect_cli)"
if [ -z "$CLI" ]; then
  cat >&2 <<'EOF'
real-agent: no coding-agent CLI found (opencode / aider / crush).

  Install one that speaks OpenRouter:
    mise use -g opencode@latest        # or: npm i -g opencode-ai
    mise use -g aider@latest           # or: pipx install aider-chat
  Then re-run.
EOF
  exit 1
fi
echo "real-agent: using $CLI @ openrouter/$MODEL, $PANES pane(s)"

# ── build the command that launches the agent on a trivial task ─────────────
# Each agent is pointed at a throwaway scratch repo with a tiny prompt so it
# genuinely runs (and shows up to muxpilot as an active agent) without doing
# anything destructive.
scratch="$(mktemp -d)"
(cd "$scratch" && git init -q && printf '# scratch\n' > README.md && git add -A && git commit -qm init) || true

agent_cmd() {
  local task="$1"
  case "$CLI" in
    opencode)
      # opencode reads OPENROUTER_API_KEY; model form openrouter/<model>.
      printf 'cd %q && OPENROUTER_API_KEY=%q opencode run --model %q %q' \
        "$scratch" "$OPENROUTER_API_KEY" "openrouter/$MODEL" "$task" ;;
    aider)
      printf 'cd %q && OPENROUTER_API_KEY=%q aider --model %q --message %q --yes-always README.md' \
        "$scratch" "$OPENROUTER_API_KEY" "openrouter/$MODEL" "$task" ;;
    crush)
      printf 'cd %q && OPENROUTER_API_KEY=%q crush run --model %q %q' \
        "$scratch" "$OPENROUTER_API_KEY" "openrouter/$MODEL" "$task" ;;
  esac
}

# ── ensure a real muxpilot binary (this path needs the real thing) ──────────
if command -v muxpilot >/dev/null 2>&1; then
  MUX="$(command -v muxpilot)"
elif [ -x "$CARGO_BIN" ]; then
  MUX="$CARGO_BIN"
elif command -v cargo >/dev/null 2>&1; then
  echo "real-agent: building release binary..."
  (cd "$ROOT_DIR" && cargo build --release -p muxpilot) && MUX="$CARGO_BIN"
else
  echo "real-agent: no muxpilot binary and no cargo to build one" >&2; exit 1
fi
export PATH="$(dirname "$MUX"):$PATH"

cleanup() {
  tmux kill-session -t "$SESSION" 2>/dev/null || true
  rm -rf "$scratch" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# ── launch the agent panes in a detached tmux session ───────────────────────
tasks=(
  "Add a one-line description under the heading in README.md"
  "List three ideas for this project as a markdown bullet list in IDEAS.md"
  "Write a short hello function in hello.sh"
)
tmux new-session -d -s "$SESSION" -n agent1 "$(agent_cmd "${tasks[0]}")"
for i in $(seq 2 "$PANES"); do
  t="${tasks[$(((i-1) % ${#tasks[@]}))]}"
  tmux new-window -t "$SESSION" -n "agent$i" "$(agent_cmd "$t")"
done

echo "real-agent: agents launching; giving them 12s to start working..."
sleep 12

# ── record the real picker showing live agents ──────────────────────────────
# Enable VHS_NO_SANDBOX where unprivileged userns is restricted (see compose.sh).
if [ -z "${VHS_NO_SANDBOX:-}" ] && \
   [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null || echo 0)" = "1" ]; then
  export VHS_NO_SANDBOX=1
fi
ensure "$MEDIA_DIR" >/dev/null
echo "real-agent: recording muxpilot picker over the live session..."
(cd "$MARKETING_DIR" && vhs tapes/real-agents.tape) || {
  echo "real-agent: vhs render failed" >&2; exit 1; }

echo "real-agent: done -> $MEDIA_DIR/real-agents.{gif,mp4}"
