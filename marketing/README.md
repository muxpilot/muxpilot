# MuxPilot marketing media

Terminal-native video pipeline for MuxPilot. Records the TUI with
[charmbracelet **VHS**](https://github.com/charmbracelet/vhs) (`.tape` scripts →
`gif`/`mp4`) so **when the UI changes we regenerate every clip with one command**.

Outputs land in [`../apps/web/public/media/`](../apps/web/public/media) and the
site embeds them at `/media/<name>` (the landing hero uses `picker.mp4`).

## Regenerate everything

```bash
make -C marketing regen
# or:
marketing/regen.sh
```

That is the whole workflow. `regen.sh` is idempotent — run it whenever the picker
changes.

### What `regen.sh` does

1. **Checks deps** — `vhs`, `ttyd`, `ffmpeg` must be on `PATH`.
2. **Resolves a `muxpilot` binary** onto `PATH`, in order of preference:
   - a `muxpilot` already on `PATH`;
   - the release binary at `target/release/muxpilot`;
   - a fresh `cargo build --release -p muxpilot` (skip with `MUXPILOT_BUILD=0`);
   - the **mock stand-in** (`scripts/mock-picker.sh`) so media still renders
     before the real `muxpilot demo` command exists.
3. **Runs every `tapes/*.tape`** through `vhs`, writing `gif` + `mp4` (+ a `png`
   poster) into the site media dir.

Render a single tape:

```bash
marketing/regen.sh picker.tape
```

## Tooling

Install VHS and its runtime deps with mise (preferred — consistent versions
across machines):

```bash
mise use -g vhs@latest ttyd@latest
```

- **ffmpeg** is also required by VHS. `apt install ffmpeg` / `brew install ffmpeg`.
- Fallback without mise: `go install github.com/charmbracelet/vhs@latest`
  (VHS still needs `ttyd` + `ffmpeg` on `PATH`).

The tapes record with **JetBrainsMono Nerd Font Mono**. If it is missing, VHS
falls back and the picker's glyphs render at the wrong width. Install it once
per machine (user-level, no root):

```bash
bash marketing/scripts/install-fonts.sh   # or: make -C marketing fonts
```

Check what is present:

```bash
make -C marketing deps
```

## Layout

```
marketing/
  regen.sh              single entry point (idempotent)
  Makefile              regen / demos / real / deps / clean
  themes/
    muxpilot.tape       shared VHS Set directives (size, font, warm theme)
  tapes/
    picker.tape         hero demo — embedded on the landing page
    filter.tape         fuzzy filtering
    navigate.tape       j/k + g/G navigation and Tab scopes
    help.tape           the ? help overlay
    real-agents.tape    records the REAL picker over a live agent session
  scripts/
    paths.sh            path constants + ensure()
    compose.sh          tape orchestrator
    narrate.sh          optional TTS narration hook
    mock-picker.sh      interactive stand-in for `muxpilot demo`
    real-agent.sh       OpenRouter live coding-agent capture
  .raw/                 git-ignored scratch (binary shim, temp)
```

The module split (`paths` / `compose` / `narrate`) mirrors a common
TypeScript video pipeline, but is terminal-native: VHS instead of Playwright.

## Deterministic demo tapes

Every demo tape drives `muxpilot demo --count 8` — a deterministic command that
launches the interactive picker populated with fixed fake sessions and agents, so
recordings are byte-stable. See the fake data and interactions in
[`scripts/mock-picker.sh`](scripts/mock-picker.sh) (the stand-in) which mirrors
what the real Rust `demo` command must render.

The shared [`themes/muxpilot.tape`](themes/muxpilot.tape) fixes terminal size
(1180×640, 26px), font (JetBrainsMono Nerd Font Mono), and a warm Tasker-ish
theme so output is deterministic and on-brand.

## Real coding-agent capture (OpenRouter)

For genuine live agent state, `scripts/real-agent.sh` launches a couple of tmux
panes each running a cheap real coding agent against **OpenRouter**, waits for
them to start working, then records the **real** picker (which reads live tmux
state) into `real-agents.gif` / `.mp4`.

```bash
export OPENROUTER_API_KEY=sk-or-...
make -C marketing real
```

- **Gated** on `OPENROUTER_API_KEY` — it skips gracefully if unset.
- **Auto-detects** an installed agent CLI: `opencode`, `aider`, or `crush`.
  Install one with `mise use -g opencode@latest` (or `npm i -g opencode-ai` /
  `pipx install aider-chat`).
- **Cheap by default**: `OPENROUTER_MODEL=qwen/qwen-2.5-coder-7b-instruct`.
  Override with `OPENROUTER_MODEL=...`, pane count with `AGENT_PANES=...`.
- Tears the demo tmux session down on exit.

## Clean

```bash
make -C marketing clean   # remove rendered media + .raw scratch
```
