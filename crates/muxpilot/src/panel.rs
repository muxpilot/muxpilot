use std::io::{Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use crate::error::{AppError, ErrorCode};
use crossterm::terminal;

use crate::snapshot::{tmux, tmux_checked, tmux_snapshot, TmuxSnapshot, FIELD_SEP, MENU_PANE_ROLE};

#[derive(Debug, Clone)]
enum PanelTarget {
    Session(String),
    Window {
        session: String,
        window_id: String,
    },
    Pane {
        session: String,
        window_id: String,
        pane_id: String,
    },
}

#[derive(Debug, Clone)]
struct PanelRow {
    label: String,
    target: PanelTarget,
}

fn panel_rows(snapshot: &TmuxSnapshot) -> Vec<PanelRow> {
    let mut rows = Vec::new();
    for session in &snapshot.sessions {
        let active = if session.name == snapshot.current_session {
            "*"
        } else {
            " "
        };
        rows.push(PanelRow {
            label: format!("{active} {}", session.name),
            target: PanelTarget::Session(session.name.clone()),
        });
        for window in &session.windows {
            let window_active = if window.active { ">" } else { " " };
            let agent_count = window.panes.iter().filter(|p| p.agent.is_some()).count();
            let agents = if agent_count == 0 {
                String::new()
            } else {
                format!("  {agent_count} agent")
            };
            rows.push(PanelRow {
                label: format!(
                    "  {window_active} {} {}{}",
                    window.index, window.name, agents
                ),
                target: PanelTarget::Window {
                    session: session.name.clone(),
                    window_id: window.id.clone(),
                },
            });
            for pane in &window.panes {
                let pane_active = if pane.active { ">" } else { " " };
                let agent = pane
                    .agent
                    .as_ref()
                    .map(|a| format!(" [{}:{}]", a.kind, a.status))
                    .unwrap_or_default();
                rows.push(PanelRow {
                    label: format!(
                        "    {pane_active} {} {}{}",
                        pane.id, pane.current_command, agent
                    ),
                    target: PanelTarget::Pane {
                        session: session.name.clone(),
                        window_id: window.id.clone(),
                        pane_id: pane.id.clone(),
                    },
                });
            }
        }
    }
    rows
}

fn panel_interactive_body(cursor: usize) -> String {
    let snapshot = tmux_snapshot();
    let rows = panel_rows(&snapshot);
    let mut lines = Vec::new();
    lines.push("tmux menu".to_string());
    lines.push(format!(
        "{}  {}",
        snapshot.current_session, snapshot.current_pane_id
    ));
    lines.push("j/k move  Enter switch  ? help  r refresh  q close".to_string());
    lines.push(String::new());
    for (idx, row) in rows.iter().enumerate() {
        let mark = if idx == cursor { ">" } else { " " };
        lines.push(format!("{mark} {}", row.label));
    }
    lines.join("\n")
}

fn panel_help_body() -> String {
    [
        "tmux menu help",
        "",
        "Navigation",
        "  j/k or arrows   move selection",
        "  Enter           switch to selected session/window/pane",
        "  r               refresh state",
        "  ?               close this help page",
        "  q or Ctrl-C     close panel",
        "",
        "Agent Status",
        "  hook            exact status from @pane_agent/@pane_status tmux options",
        "  process         inferred from pane process tree",
        "  pane-command    inferred from tmux pane_current_command",
        "  detected        agent found, precise state unknown",
        "",
        "Symbols",
        "  *               current session",
        "  >               active window/pane or selected help row",
        "  [agent:status]  detected agent state",
        "",
        "CLI",
        "  muxpilot state",
        "  muxpilot state --json",
        "  muxpilot toggle-panel",
    ]
    .join("\n")
}

fn panel_switch(target: &PanelTarget) {
    match target {
        PanelTarget::Session(name) => {
            let _ = tmux(&["switch-client", "-t", name]);
        }
        PanelTarget::Window { session, window_id } => {
            let _ = switch_tmux_window(session, window_id);
        }
        PanelTarget::Pane {
            session,
            window_id,
            pane_id,
        } => {
            let _ = switch_tmux_pane(session, window_id, pane_id);
        }
    }
}

/// Bring the calling client to `session`. Targets the specific client tty when
/// we can resolve it (so it works from inside a `display-popup`), else the
/// ambient client.
fn switch_client_to(session: &str, operation: &str) -> Result<(), AppError> {
    let client = tmux(&["display-message", "-p", "#{client_tty}"]);
    if client.is_empty() {
        tmux_checked(&["switch-client", "-t", session], operation)
    } else {
        tmux_checked(&["switch-client", "-c", &client, "-t", session], operation)
    }
}

pub(crate) fn switch_tmux_window(session: &str, window_id: &str) -> Result<(), AppError> {
    // Make the window active in its session *first*, then bring the client over.
    // `switch-client` adopts whatever window is active in the target session, so
    // doing it last lands the client on this window; doing it first (the old
    // behaviour) landed on the session's previously-active window.
    tmux_checked(&["select-window", "-t", window_id], "muxpilot.switch-window")?;
    switch_client_to(session, "muxpilot.switch-window")
}

pub(crate) fn switch_tmux_pane(
    session: &str,
    window_id: &str,
    pane_id: &str,
) -> Result<(), AppError> {
    // Set the target window and pane active in their session first, then switch
    // the client to the session — it adopts the now-active window/pane, landing
    // exactly on this agent's pane rather than the session's last-used window.
    tmux_checked(&["select-window", "-t", window_id], "muxpilot.switch-pane")?;
    tmux_checked(&["select-pane", "-t", pane_id], "muxpilot.switch-pane")?;
    switch_client_to(session, "muxpilot.switch-pane")
}

fn sidebar_width() -> String {
    let client_width = tmux(&["display-message", "-p", "#{client_width}"])
        .parse::<u32>()
        .unwrap_or(100);
    let width = (client_width / 3).clamp(32, 56);
    width.to_string()
}

struct RawModeGuard {
    original: Option<String>,
}

impl RawModeGuard {
    fn enter() -> Self {
        let original = std::process::Command::new("stty")
            .arg("-g")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty());
        let _ = std::process::Command::new("stty")
            .args(["raw", "-echo"])
            .status();
        Self { original }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            let _ = std::process::Command::new("stty").arg(original).status();
        } else {
            let _ = std::process::Command::new("stty").args(["sane"]).status();
        }
    }
}

pub(crate) fn run_watch_panel() -> Result<ExitCode, AppError> {
    let _raw = RawModeGuard::enter();
    let mut cursor = 0usize;
    let mut show_help = false;
    loop {
        let body = if show_help {
            panel_help_body()
        } else {
            panel_interactive_body(cursor)
        };
        let visible_body = match terminal::size() {
            Ok((_, rows)) => body
                .lines()
                .take(rows as usize)
                .collect::<Vec<_>>()
                .join("\r\n"),
            Err(_) => body.replace('\n', "\r\n"),
        };
        print!("\x1b[2J\x1b[H{visible_body}");
        std::io::stdout().flush().map_err(|e| {
            AppError::new(
                ErrorCode::ProviderFailure,
                format!("failed to flush panel: {e}"),
            )
        })?;

        let snapshot = tmux_snapshot();
        let rows = panel_rows(&snapshot);
        if rows.is_empty() {
            cursor = 0;
        } else if cursor >= rows.len() {
            cursor = rows.len() - 1;
        }

        let mut buf = [0u8; 1];
        if std::io::stdin().read_exact(&mut buf).is_err() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        match buf[0] {
            b'q' | 3 => return Ok(ExitCode::SUCCESS),
            b'?' => show_help = !show_help,
            b'j' => {
                if show_help {
                    continue;
                }
                if !rows.is_empty() {
                    cursor = (cursor + 1).min(rows.len() - 1);
                }
            }
            b'k' => {
                if show_help {
                    continue;
                }
                cursor = cursor.saturating_sub(1);
            }
            b'r' => {}
            b'\n' | b'\r' => {
                if show_help {
                    show_help = false;
                    continue;
                }
                if let Some(row) = rows.get(cursor) {
                    panel_switch(&row.target);
                }
            }
            27 => {
                if show_help {
                    show_help = false;
                    continue;
                }
                let mut seq = [0u8; 2];
                if std::io::stdin().read_exact(&mut seq).is_ok() {
                    match seq {
                        [91, 65] => cursor = cursor.saturating_sub(1),
                        [91, 66] => {
                            if !rows.is_empty() {
                                cursor = (cursor + 1).min(rows.len() - 1);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn toggle_panel() -> Result<ExitCode, AppError> {
    let window_id = tmux(&["display-message", "-p", "#{window_id}"]);
    if window_id.is_empty() {
        return Ok(ExitCode::FAILURE);
    }

    let panes = tmux(&[
        "list-panes",
        "-t",
        &window_id,
        "-F",
        "#{pane_id}::__TMUX_MENU__:#{@pane_role}",
    ]);
    for line in panes.lines() {
        let parts: Vec<&str> = line.split(FIELD_SEP).collect();
        if parts.len() == 2 && parts[1] == MENU_PANE_ROLE {
            let _ = tmux(&["kill-pane", "-t", parts[0]]);
            return Ok(ExitCode::SUCCESS);
        }
    }

    let active_pane = tmux(&["display-message", "-p", "#{pane_id}"]);
    let current_path = tmux(&["display-message", "-p", "#{pane_current_path}"]);
    let self_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "muxpilot".to_string());
    let pane = tmux(&[
        "split-window",
        "-hfb",
        "-l",
        &sidebar_width(),
        "-c",
        &current_path,
        "-P",
        "-F",
        "#{pane_id}",
        &self_bin,
        "panel",
    ]);
    if !pane.is_empty() {
        let _ = tmux(&["set-option", "-pt", &pane, "@pane_role", MENU_PANE_ROLE]);
    }
    if !active_pane.is_empty() {
        let _ = tmux(&["select-pane", "-t", &active_pane]);
    }
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// Discovery (side-effecting; reuses std process and filesystem)
// ---------------------------------------------------------------------------
