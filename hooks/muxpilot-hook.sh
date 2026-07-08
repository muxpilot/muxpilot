#!/usr/bin/env bash
# MuxPilot agent-state hook.
#
# Stamps the current tmux pane's user-options so `muxpilot` can show live agent
# state at confidence 100 — no screen-scraping needed. Wire it into a coding
# agent's lifecycle hooks (see claude-settings.json / codex-hooks.json).
#
#   muxpilot-hook.sh <EVENT>
#
# <EVENT> is the lifecycle event name (SessionStart, UserPromptSubmit, PreToolUse,
# PostToolUse, Notification, Stop, StopFailure, SubagentStart, SubagentStop,
# SessionEnd). The full hook JSON is read from stdin when present.
#
# Design notes:
#  * Never fails the agent — every tmux call is best-effort and the script always
#    exits 0 (a non-zero hook exit can block the agent's tool call).
#  * Staleness guard: racing parallel hook subprocesses can't clobber newer state
#    with older, via a millisecond timestamp in @pane_status_ts.
#  * Un-wait on tool activity: a tool running clears a prior waiting state.
#  * idle_prompt notifications are metadata only — they do NOT raise attention.
#  * Subagents share the parent's $TMUX_PANE, so the "done" transition is deferred
#    until the last subagent stops.

pane="${TMUX_PANE:-}"
[ -n "$pane" ] || exit 0
command -v tmux >/dev/null 2>&1 || exit 0

event="${1:-}"
agent="${MUXPILOT_AGENT:-claude}"
now_ms=$(( $(date +%s%N 2>/dev/null || echo 0) / 1000000 ))
payload="$(cat 2>/dev/null || true)"

get() { tmux show-options -pqv -t "$pane" "$1" 2>/dev/null || true; }
put() { tmux set-option -p -t "$pane" "$1" "$2" 2>/dev/null || true; }
unset_opt() { tmux set-option -pu -t "$pane" "$1" 2>/dev/null || true; }

# Staleness guard — drop events older than the last one recorded.
last_ts="$(get @pane_status_ts)"
if [ -n "$last_ts" ] && [ "$now_ms" -gt 0 ] && [ "$now_ms" -lt "$last_ts" ]; then
  exit 0
fi

subs="$(get @pane_subagents)"; subs="${subs:-0}"
case "$subs" in *[!0-9]*) subs=0 ;; esac

status=""; attention=""; wait_reason=""
case "$event" in
  SessionStart)
    status="working"; attention="clear" ;;
  UserPromptSubmit|PreToolUse|PostToolUse)
    # Any prompt/tool activity means the agent is running — clear a prior wait.
    status="working"; attention="clear" ;;
  SubagentStart)
    subs=$((subs + 1)); put @pane_subagents "$subs"; status="working" ;;
  SubagentStop)
    subs=$(( subs > 0 ? subs - 1 : 0 )); put @pane_subagents "$subs" ;;
  Notification)
    case "$payload" in
      *permission_prompt*)  status="waiting-approve"; attention="1"; wait_reason="permission" ;;
      *idle_prompt*)        status="idle"; attention="clear" ;;
      *)                    : ;;
    esac ;;
  Stop)
    # Don't declare done while subagents (which share this pane) are still live.
    [ "$subs" -gt 0 ] && exit 0
    status="idle"; attention="clear" ;;
  StopFailure)
    case "$payload" in
      *rate_limit*)  status="rate-limited"; attention="1"; wait_reason="rate limit" ;;
      *)             status="error"; attention="1"; wait_reason="turn failed" ;;
    esac ;;
  SessionEnd)
    unset_opt @pane_agent; unset_opt @pane_status; unset_opt @pane_attention
    unset_opt @pane_wait_reason; unset_opt @pane_status_ts; unset_opt @pane_subagents
    exit 0 ;;
  *) : ;;
esac

put @pane_agent "$agent"
[ -n "$status" ]      && put @pane_status "$status"
[ -n "$attention" ]   && put @pane_attention "$attention"
[ -n "$wait_reason" ] && put @pane_wait_reason "$wait_reason"
[ "$now_ms" -gt 0 ]   && put @pane_status_ts "$now_ms"
exit 0
