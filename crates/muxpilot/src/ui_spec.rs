//! Hidden `muxpilot ui-spec` maintainer command.
//!
//! Runs the **real** per-mode entry builders (`build_session_entries`,
//! `build_agent_entries`, `build_layout_entries`, `build_directory_entries`)
//! over a fixed synthetic fixture and serializes exactly what each picker mode
//! would render, as JSON. This is the machine-checkable "render contract" that
//! keeps external mockups (the Claude Design system) from drifting from the
//! actual app.
//!
//! IMPORTANT: the `demo` command is NOT authoritative for per-mode layout — it
//! builds one merged list and fakes modes by filtering, so its Sessions view
//! wrongly shows a TMUXINATOR group and its Agents view uses the wrong grouping.
//! `ui-spec` calls the same builders the interactive picker does, so it is.
//!
//! Timestamps are intentionally `None` so the "last activity" column renders a
//! stable `-` and the output is deterministic (a diff-able contract). The design
//! screens show illustrative times; only glyph/name/model/status/group are
//! governed by this contract.

use std::process::ExitCode;

use crate::cli_format::print_json;
use crate::error::AppError;
use crate::model::{DirItem, Layout, MenuModel};
use crate::native_state::NativeEntry;
use crate::snapshot::{
    AgentState, AgentStateSource, PaneAgentStatus, TmuxPane, TmuxSession, TmuxSnapshot, TmuxWindow,
};
use crate::workspace_entries::{
    build_agent_entries, build_directory_entries, build_layout_entries, build_session_entries,
};

fn agent(kind: &str, model: &str, status: PaneAgentStatus, attention: bool) -> AgentState {
    AgentState {
        kind: kind.to_string(),
        status,
        source: AgentStateSource::Process,
        confidence: 84,
        attention,
        wait_reason: String::new(),
        model: Some(model.to_string()),
        evidence: vec!["process".to_string()],
        is_active: status == PaneAgentStatus::Working,
        last_change: None,
    }
}

fn pane(id: &str, cmd: &str, agent: Option<AgentState>) -> TmuxPane {
    TmuxPane {
        id: id.to_string(),
        active: false,
        path: "/home/user/code".to_string(),
        current_command: cmd.to_string(),
        pid: Some(1),
        last_activity: None,
        role: String::new(),
        agent,
    }
}

fn window(id: &str, index: u32, name: &str, active: bool, panes: Vec<TmuxPane>) -> TmuxWindow {
    TmuxWindow {
        id: id.to_string(),
        index,
        name: name.to_string(),
        active,
        last_activity: None,
        panes,
    }
}

fn session(name: &str, windows: Vec<TmuxWindow>) -> TmuxSession {
    TmuxSession {
        name: name.to_string(),
        windows,
    }
}

/// A small, deterministic fixture exercising each mode: a plain session, a
/// multi-window multi-agent session (multi-pane + inline one-pane windows), an
/// attention (waiting-approve) agent, tmuxinator layouts (running + stopped),
/// and directories (configured + bare).
fn fixture() -> (MenuModel, TmuxSnapshot) {
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "synthetic",
        backend: "synthetic",
        current_session: "web-dashboard".to_string(),
        current_window_id: "@0".to_string(),
        current_pane_id: "%0".to_string(),
        sessions: vec![
            session(
                "web-dashboard",
                vec![window("@0", 0, "server", true, vec![pane("%0", "node", None)])],
            ),
            session(
                "api-gateway",
                vec![
                    window(
                        "@1",
                        0,
                        "editor",
                        true,
                        vec![
                            pane(
                                "%1",
                                "codex",
                                Some(agent("codex", "opus-4-8", PaneAgentStatus::Working, false)),
                            ),
                            pane("%2", "zsh", None),
                        ],
                    ),
                    window(
                        "@2",
                        1,
                        "server",
                        false,
                        vec![pane(
                            "%3",
                            "cline",
                            Some(agent("cline", "sonnet-5", PaneAgentStatus::Working, false)),
                        )],
                    ),
                    window("@3", 2, "logs", false, vec![pane("%4", "tail", None)]),
                ],
            ),
            session(
                "payments-service",
                vec![window(
                    "@4",
                    0,
                    "review",
                    true,
                    vec![pane(
                        "%5",
                        "claude",
                        Some(agent(
                            "claude",
                            "opus-4-8",
                            PaneAgentStatus::WaitingApprove,
                            true,
                        )),
                    )],
                )],
            ),
            session(
                "data-pipeline",
                vec![
                    window("@5", 0, "shell", true, vec![pane("%6", "zsh", None)]),
                    window("@6", 1, "logs", false, vec![pane("%7", "tail", None)]),
                ],
            ),
        ],
    };

    let model = MenuModel {
        current: "web-dashboard".to_string(),
        layouts: vec![
            Layout {
                session: "web-dashboard".to_string(),
                display: "~/code/web-dashboard".to_string(),
                path: "/home/user/code/web-dashboard".to_string(),
                running: true,
            },
            Layout {
                session: "auth-service".to_string(),
                display: "~/code/auth-service".to_string(),
                path: "/home/user/code/auth-service".to_string(),
                running: false,
            },
            Layout {
                session: "cms-backend".to_string(),
                display: "~/code/cms-backend".to_string(),
                path: "/home/user/code/cms-backend".to_string(),
                running: false,
            },
        ],
        zoxide: vec![
            DirItem {
                display: "~/code/web-dashboard".to_string(),
                path: "/home/user/code/web-dashboard".to_string(),
                has_local_config: true,
            },
            DirItem {
                display: "~/downloads".to_string(),
                path: "/home/user/downloads".to_string(),
                has_local_config: false,
            },
        ],
        plain_repos: vec![DirItem {
            display: "~/gits/github/acme/infra".to_string(),
            path: "/home/user/gits/github/acme/infra".to_string(),
            has_local_config: false,
        }],
        ..Default::default()
    };

    (model, snapshot)
}

/// The animated braille spinner (`ui::text::spinner_frame`) is time-based, so
/// collapse every frame to a canonical `⠋` — otherwise the "working" status
/// cell would make the contract non-deterministic between runs.
fn normalize_spinner(line: &str) -> String {
    line.chars()
        .map(|c| match c {
            '⠙' | '⠹' | '⠸' | '⠼' | '⠴' | '⠦' | '⠧' => '⠋',
            other => other,
        })
        .collect()
}

/// Serialize one mode's entries, grouped by their `NativeGroup` label in the
/// order the builder emitted them.
fn mode_json(mode: &str, entries: &[NativeEntry]) -> serde_json::Value {
    let mut groups: Vec<(&'static str, Vec<serde_json::Value>)> = Vec::new();
    for entry in entries {
        let label = entry.group.label();
        let row = serde_json::json!({
            "line": normalize_spinner(&entry.line),
            "tags": entry.tags,
            "detail": entry.detail.lines().map(normalize_spinner).collect::<Vec<_>>(),
        });
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, rows)) => rows.push(row),
            None => groups.push((label, vec![row])),
        }
    }
    let groups: Vec<serde_json::Value> = groups
        .into_iter()
        .map(|(label, rows)| serde_json::json!({ "label": label, "rows": rows }))
        .collect();
    serde_json::json!({ "mode": mode, "count": entries.len(), "groups": groups })
}

/// Entry point for `muxpilot ui-spec`. Always emits JSON — it is a machine
/// contract, not a human view.
pub(crate) fn run_ui_spec() -> Result<ExitCode, AppError> {
    let (model, snap) = fixture();
    let modes = serde_json::json!([
        mode_json("sessions", &build_session_entries(&model, &snap)),
        mode_json("agents", &build_agent_entries(&snap)),
        mode_json("layouts", &build_layout_entries(&model, &snap)),
        mode_json("dirs", &build_directory_entries(&model)),
    ]);
    let doc = serde_json::json!({
        "schema": "muxpilot.ui-spec.v1",
        "version": env!("CARGO_PKG_VERSION"),
        "note": "Rendered by the real per-mode builders over a synthetic fixture. \
                 Authoritative for grouping and row content; the `demo` command is not.",
        "modes": modes,
    });
    print_json(doc, "muxpilot.ui-spec.json")?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_mode_is_running_only() {
        // Guards the exact regression the Claude Design mockups hit: tmuxinator
        // layouts must NOT appear in Sessions (they live only in Layouts).
        let (model, snap) = fixture();
        let entries = build_session_entries(&model, &snap);
        assert!(!entries.is_empty());
        let labels: Vec<_> = entries.iter().map(|e| e.group.label()).collect();
        assert!(
            labels.iter().all(|l| *l == "RUNNING"),
            "Sessions must be running-only, got groups {labels:?}"
        );
    }

    #[test]
    fn agents_mode_groups_by_attention() {
        let (_, snap) = fixture();
        let labels: Vec<_> = build_agent_entries(&snap)
            .iter()
            .map(|e| e.group.label())
            .collect();
        assert!(labels.contains(&"NEEDS YOU"), "got {labels:?}");
        assert!(labels.contains(&"WORKING"), "got {labels:?}");
    }

    #[test]
    fn layouts_hold_the_tmuxinator_files() {
        let (model, snap) = fixture();
        let labels: Vec<_> = build_layout_entries(&model, &snap)
            .iter()
            .map(|e| e.group.label())
            .collect();
        assert!(labels.iter().all(|l| *l == "TMUXINATOR"), "got {labels:?}");
    }

    #[test]
    fn spinner_normalization_is_stable() {
        assert_eq!(normalize_spinner("● x · ⠼ work · -"), "● x · ⠋ work · -");
        assert_eq!(normalize_spinner("● x · ⠹ work · -"), "● x · ⠋ work · -");
    }
}
