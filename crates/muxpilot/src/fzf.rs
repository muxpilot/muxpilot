pub const FZF_HEADER: &str =
    "Tmux Menu  🟢 current  📺 session  🚀 layout ▶ running  🎬 global  ⭐ zoxide  📁 git";
pub const FZF_FOOTER: &str =
    "Enter switch/start  ?/F1/Ctrl-H help  Ctrl-G rows  Ctrl-/ preview  Ctrl-R reload  Esc cancel";
pub const FZF_HELP: &str = r#"tmux menu help

Navigation
  Type            fuzzy filter
  Up/Down         move selection
  Enter           switch to session or start selected project/layout
  Esc             cancel
  Ctrl-R          reload menu rows
  Ctrl-/          toggle preview pane
  ? / F1 / Ctrl-H show this help
  Ctrl-G          return preview to selected row

Rows
  🟢 current      current tmux session
  📺 session      running tmux session; right side shows window count
  🚀 layout       repo-local tmuxinator layout; ▶ means already running
  🎬 global       saved tmuxinator project
  ⭐ zoxide       frecent directory
  📁 git          git checkout without local tmux layout

Agent state
  Hook metadata wins when present.
  Without hooks, state is inferred from tmux pane command and process tree.

Other commands
  muxpilot state
  muxpilot state --json
  muxpilot toggle-panel
"#;

pub fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
