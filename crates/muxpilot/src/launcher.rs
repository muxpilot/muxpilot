use std::path::Path;

use crate::error::{AppError, ErrorCode};

use crate::discovery::{agentvibes_layout, in_tmux, tmux_sessions};
use crate::model::{sanitize_session_name, Selection};
use crate::panel::{switch_tmux_pane, switch_tmux_window};
use crate::snapshot::tmux;

fn run_interactive(args: &[&str], cwd: Option<&str>) -> Result<(), AppError> {
    let mut cmd = std::process::Command::new(args[0]);
    cmd.args(&args[1..]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.status().map(|_| ()).map_err(|e| {
        AppError::new(
            ErrorCode::ProviderFailure,
            format!("failed to launch {}", args[0]),
        )
        .op("muxpilot.launch")
        .with_source(e)
    })
}

/// Attach to (outside tmux) or switch-client to (inside tmux) an existing session.
pub(crate) fn enter_session(name: &str) -> Result<(), AppError> {
    if in_tmux() {
        run_interactive(&["tmux", "switch-client", "-t", name], None)
    } else {
        run_interactive(&["tmux", "attach", "-t", name], None)
    }
}

/// Start a repo-local layout, preferring `.agentvibes/tmux.yml`.
fn start_local_layout(path: &str) -> Result<(), AppError> {
    if Path::new(&agentvibes_layout(path)).is_file() {
        run_interactive(
            &["tmuxinator", "start", "-p", ".agentvibes/tmux.yml"],
            Some(path),
        )
    } else {
        run_interactive(&["tmuxinator", "local"], Some(path))
    }
}

pub(crate) fn target_session_window(target: &str) -> Option<(String, String)> {
    let raw = tmux(&[
        "display-message",
        "-p",
        "-t",
        target,
        "#{session_name}\t#{window_id}",
    ]);
    let (session, window_id) = raw.split_once('\t')?;
    if session.is_empty() || window_id.is_empty() {
        None
    } else {
        Some((session.to_string(), window_id.to_string()))
    }
}

pub(crate) async fn execute(selection: Selection) -> Result<(), AppError> {
    match selection {
        Selection::Layout { session, full_path } => {
            if tmux_sessions().await.iter().any(|s| s == &session) {
                enter_session(&session)
            } else {
                start_local_layout(&full_path)
            }
        }
        Selection::Session(name) => enter_session(&name),
        Selection::Window { session, window_id } => switch_tmux_window(&session, &window_id),
        Selection::Pane {
            session,
            window_id,
            pane_id,
        } => switch_tmux_pane(&session, &window_id, &pane_id),
        Selection::Project(name) => run_interactive(&["tmuxinator", "start", &name], None),
        Selection::Dir {
            full_path,
            has_local_config,
        } => {
            if has_local_config {
                start_local_layout(&full_path)
            } else {
                let session = sanitize_session_name(&full_path);
                if tmux_sessions().await.iter().any(|s| s == &session) {
                    enter_session(&session)
                } else {
                    run_interactive(
                        &[
                            "tmux",
                            "new-session",
                            "-d",
                            "-s",
                            &session,
                            "-c",
                            &full_path,
                        ],
                        None,
                    )?;
                    enter_session(&session)
                }
            }
        }
    }
}
