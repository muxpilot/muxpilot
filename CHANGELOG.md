# Changelog

All notable changes to MuxPilot are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-07-09

First feature release after the initial launch — a much richer picker plus a
batch of fixes reported against 0.1.0.

### Added

- **Picker modes** — footer-switched **Sessions / Agents / Layouts / Dirs**, each
  a focused view over the same workspace inventory.
- **Three-level tree** — expand a running session into its windows, and a window
  into its panes. Windows that host a coding agent **auto-reveal that pane** —
  showing its agent, model, and live status inline — the moment the session is
  expanded, no second keystroke; `l` on a window still opens its remaining shell
  panes. The help overlay legend is now scrollable.
- **Per-pane agent model** — an agent pane shows its `kind + model`
  (e.g. `claude opus-4-8`). Resolved from the `@pane_model` hook (authoritative),
  a `--model`/`-m` arg, or the process environment (`ANTHROPIC_MODEL`, …) as an
  honest best-effort guess.
- **Agent state from the screen** — panes are classified `working` / `waiting` /
  `idle` with trustworthy last-activity, a fleet summary, and severity bubbling
  to the workspace row. Copy-paste agent-state hooks ship in `hooks/`.
- **`cline`** is now recognized as its own agent (previously mislabeled `claude`
  when it spawned a Claude child process).
- **Localization scaffolding** (i18n strings module) and a configurable keymap
  table.
- The picker status bar now shows the running version (`muxpilot vX.Y.Z`).
- **`--version` / `-V`** prints `muxpilot <semver>` — handy for scripts, and the
  smoke test package managers (mise, etc.) expect.
- **Layouts show their path** — the Layouts view renders the tilde-collapsed path
  each layout/project is stored at, middle-elided when it must clip (so the `~/`
  root and the name tail both survive), with the full absolute yaml path in the
  detail pane.

### Changed

- The picker now opens with the cursor on the **current (`◆`) workspace** instead
  of the top row.
- The **`CONFIGURED` group is renamed `TMUXINATOR`**; the status column names the
  specific kind (`layout` / `project`) instead of echoing the header, and the
  detail pane shows a `Path:` line wherever a location is known.
- `l` / `→` toggles the session tree instead of only expanding it.
- Docs site: a compact **Get started** section with brew / cargo / npm / tmux
  install paths, and footer links to GitHub / crates.io / npm.

### Fixed

- The **agent activity spinner now animates** in the picker (it was frozen — the
  frame was baked into the row at build time and never refreshed).
- Hook working-state and pane-screen detection robustness.

## [0.1.0] - 2026-07-08

Initial public release.

- Fast native tmux workspace picker: active sessions, repo-local tmuxinator
  layouts, saved tmuxinator projects, zoxide-frecent directories, and plain git
  repositories in one list.
- Agent-aware: panes running coding agents are detected and surfaced inline.
- Distribution via crates.io, npm (`muxpilot`), Homebrew
  (`brew install muxpilot/tap/muxpilot`), shell installer, and GitHub Releases,
  wired through cargo-dist.

[0.1.1]: https://github.com/muxpilot/muxpilot/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/muxpilot/muxpilot/releases/tag/v0.1.0
