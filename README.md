# MuxPilot

MuxPilot is a fast Rust tmux workspace picker and agent-aware session menu.

It discovers everything launchable — active tmux sessions, repo-local
tmuxinator layouts, saved tmuxinator projects, zoxide frecent directories, and
plain git repositories — renders them in a single native picker, and switches to
or starts whatever you choose. Running sessions expand into a tree of their
windows, and panes running coding agents are detected and surfaced inline.

## Install

```bash
cargo install muxpilot                # crates.io
brew install muxpilot/tap/muxpilot    # Homebrew
npm i -g muxpilot                     # npm (or: npx muxpilot)
```

Prebuilt binaries and a shell installer are on the [releases page][releases].
Site: <https://muxpilot.n.yatsyk.com>

[releases]: https://github.com/muxpilot/muxpilot/releases/latest

## Usage

```bash
muxpilot                 # open the interactive picker
muxpilot --help          # command surface
muxpilot state --json    # dump the resolved tmux state
```

Bind it to a tmux key for an instant popup:

```tmux
bind-key C-j display-popup -E -w 80% -h 70% "muxpilot"
```

## Layout

- `crates/muxpilot` — the Rust CLI and native tmux picker.
- `apps/web` — the product/docs website (Next.js).

## Development

```bash
cargo test
cargo clippy -- -D warnings
cargo run -p muxpilot -- --help
```

## License

MIT
