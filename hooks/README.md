# MuxPilot agent-state hooks

MuxPilot detects agent state per pane on a confidence ladder: **hook metadata
(100)** → capture-pane screen classification (55–80) → process tree (85) →
`pane_current_command` (60). Everything below the hook tier is inference. Wiring
these hooks makes your agents *report* their state directly, so MuxPilot shows
`working` / `waiting-approve` / `idle` / `error` with certainty and zero
screen-scraping.

## What it does

Each lifecycle event runs [`muxpilot-hook.sh`](./muxpilot-hook.sh), which stamps
the agent's tmux pane with user-options MuxPilot reads:

| Option | Meaning |
|---|---|
| `@pane_agent` | which agent (`claude` / `codex` / …) |
| `@pane_status` | `working` / `waiting-approve` / `waiting-input` / `idle` / `error` / `rate-limited` |
| `@pane_attention` | `1` = needs you, `clear` = doesn't |
| `@pane_wait_reason` | short reason string |
| `@pane_model` | model slug the agent is running (`claude-opus-4-8`, `gpt-5.5`) |
| `@pane_status_ts` | ms timestamp — staleness guard for racing hooks |
| `@pane_subagents` | live subagent count — defers "done" |

The script is **best-effort and always exits 0** — a hook must never block the
agent.

## Install

```bash
mkdir -p ~/.muxpilot
cp hooks/muxpilot-hook.sh ~/.muxpilot/
chmod +x ~/.muxpilot/muxpilot-hook.sh
```

**Claude Code** — merge [`claude-settings.json`](./claude-settings.json)'s
`hooks` block into `~/.claude/settings.json` (the user-level
`settings.local.json` is **not** read). Hooks hot-reload, so running sessions
start reporting immediately.

**Codex CLI** — copy [`codex-hooks.json`](./codex-hooks.json) to
`~/.codex/hooks.json` (Codex **auto-discovers** that path — there is no
`hooks_file` key). Ensure `[features] hooks = true` in `~/.codex/config.toml`
(on by default). Then **run `/hooks` inside Codex to review and *trust* the
entries** — untrusted command hooks silently never fire (`--dangerously-bypass-hook-trust`
skips the gate for automation). Prefer the user-level `~/.codex/hooks.json` over
repo-local `.codex/config.toml` (see `openai/codex#17532`). Codex's `PostToolUse`
is Bash-only, but it fires `PermissionRequest` (→ waiting-approve) and sends the
model on every event, so Codex panes get reliable waiting + model without
screen-scraping.

## Model detection

The hook stamps `@pane_model` from the payload's `model` field. **Codex** sends
it on every event (always populated). **Claude Code** sends `model` only on
`SessionStart` (so it's set once per session). When the hook option is absent,
MuxPilot falls back to the agent's process args (`--model` / `-m`). Shown in the
picker preview and `muxpilot state`.

## Verify

```bash
# In a tmux pane, simulate an event and read it back:
TMUX_PANE=$(tmux display -p '#{pane_id}') bash ~/.muxpilot/muxpilot-hook.sh UserPromptSubmit </dev/null
tmux show-options -pv @pane_status      # -> working
muxpilot state --json | jq '.sessions[].windows[].panes[].agent | select(.source=="hook")'
```

## Hardening built in

- **Staleness guard** — parallel hook subprocesses race; an event older than the
  last recorded `@pane_status_ts` is dropped.
- **Un-wait on tool activity** — a `PreToolUse`/`PostToolUse` clears a prior
  waiting state (the agent self-resumed).
- **`idle_prompt` is metadata-only** — "done, ready for next prompt" does not
  raise attention (no false "needs you").
- **Subagent-aware** — subagents share the parent's `$TMUX_PANE`, so `Stop`
  defers the `idle` transition until the last subagent stops.

## Security

MuxPilot only ever **reads** these options and switches you to the pane — it
never injects keystrokes into an agent. The hooks only write tmux-local pane
metadata; they carry no secrets and reach nothing outside your tmux server.
