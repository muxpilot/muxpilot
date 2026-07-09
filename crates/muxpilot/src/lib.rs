//! MuxPilot — unified tmux session / project launcher.
//!
//! A standalone Rust tmux menu. It discovers
//! launchable things — active tmux sessions, repo-local tmuxinator layouts
//! (`.agentvibes/tmux.yml` / `.tmuxinator.yml`), saved global tmuxinator
//! projects, zoxide frecent directories, and plain git repositories — renders a
//! single emoji-prefixed menu, lets the user pick one (via `fzf`, or
//! `--list` to just print it), and launches the choice.
//!
//! Interactive terminal hand-off (`fzf`, `tmux attach`, `tmuxinator start`)
//! uses `std::process::Command` directly.

use std::process::ExitCode;

use crate::error::{AppError, ErrorCode};

mod cli_format;
pub use cli_format::args_want_json;
use cli_format::{output_format, print_json, OutputFormat};

mod error;

mod model;
pub use model::*;

mod discovery;
use discovery::build_model_cached;
pub(crate) use discovery::home;

mod launcher;
use launcher::{enter_session, execute, target_session_window};

mod native_state;
#[cfg(test)]
pub(crate) use native_state::NativeEntry;

mod demo;
#[cfg(test)]
pub(crate) use demo::build_demo_entries;

mod fzf;
use fzf::{shell_single_quote, FZF_FOOTER, FZF_HEADER, FZF_HELP};

mod keymap;

mod native_picker;
use native_picker::select_native;

mod native_view;

mod picker_state;

mod panel;
use panel::{run_watch_panel, switch_tmux_pane, switch_tmux_window, toggle_panel};

mod snapshot;
use snapshot::{
    command_output, render_snapshot_human, tmux, tmux_snapshot_with_options, SnapshotOptions,
    TmuxSnapshot,
};
#[cfg(test)]
pub(crate) use snapshot::{
    AgentState, AgentStateSource, PaneAgentStatus, TmuxPane, TmuxSession, TmuxWindow,
};

mod ui;
#[cfg(test)]
pub(crate) use ui::{
    display_width, entry_columns, entry_glyph, entry_header, entry_matches, picker_body_range,
    picker_body_rows, picker_uses_compact_height,
};

mod workspace_entries;
#[cfg(test)]
pub(crate) use workspace_entries::{
    build_agent_entries, build_directory_entries, build_layout_entries, build_native_entries,
    build_session_entries, workspace_detail, WindowSummary, WorkspaceRow,
};

pub(crate) async fn select_with_fzf(menu: &str) -> Result<Option<String>, AppError> {
    let help_preview = format!("printf %s {}", shell_single_quote(FZF_HELP));
    let row_preview = "echo {}".to_string();
    let toggle_help = format!("change-preview({help_preview})+change-preview-window(right:50%)");
    let toggle_row = format!("change-preview({row_preview})+change-preview-window(right:50%)");
    let help_bind = format!("?:{toggle_help},f1:{toggle_help},ctrl-h:{toggle_help}");
    let row_bind = format!("ctrl-g:{toggle_row}");
    let output = std::process::Command::new("fzf")
        .args([
            "--reverse",
            "--border=none",
            "--prompt=tmux > ",
            "--info=inline-right",
            &format!("--header={FZF_HEADER}"),
            "--header-first",
            &format!("--footer={FZF_FOOTER}"),
            "--footer-border=top",
            "--preview-window=right:50%",
            &format!("--preview={row_preview}"),
            "--bind=ctrl-/:toggle-preview",
            "--bind=ctrl-r:reload(muxpilot --list)",
            &format!("--bind={help_bind}"),
            &format!("--bind={row_bind}"),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(menu.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(|e| {
            AppError::new(
                ErrorCode::ProviderFailure,
                format!("failed to launch fzf: {e}"),
            )
            .op("muxpilot.fzf")
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    let choice = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if choice.is_empty() {
        None
    } else {
        Some(choice)
    })
}

fn render_agents_human(snapshot: &TmuxSnapshot) -> String {
    let mut lines = Vec::new();
    for session in &snapshot.sessions {
        for window in &session.windows {
            for pane in &window.panes {
                let Some(agent) = &pane.agent else {
                    continue;
                };
                lines.push(format!(
                    "{}:{} {} {}:{} model={} confidence={} source={:?} attention={} wait={}",
                    session.name,
                    window.index,
                    pane.id,
                    agent.kind,
                    agent.status,
                    agent.model.as_deref().unwrap_or("-"),
                    agent.confidence,
                    agent.source,
                    agent.attention,
                    if agent.wait_reason.is_empty() {
                        "-"
                    } else {
                        &agent.wait_reason
                    }
                ));
            }
        }
    }
    if lines.is_empty() {
        "no agent panes detected".to_string()
    } else {
        lines.join("\n")
    }
}

fn agent_rows(snapshot: &TmuxSnapshot) -> Vec<serde_json::Value> {
    snapshot
        .sessions
        .iter()
        .flat_map(|session| {
            session.windows.iter().flat_map(move |window| {
                window.panes.iter().filter_map(move |pane| {
                    pane.agent.as_ref().map(|agent| {
                        serde_json::json!({
                            "session": session.name,
                            "window_id": window.id,
                            "window_index": window.index,
                            "window_name": window.name,
                            "pane_id": pane.id,
                            "path": pane.path,
                            "command": pane.current_command,
                            "agent": agent,
                        })
                    })
                })
            })
        })
        .collect()
}

fn menu_rows_json(model: &MenuModel) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();

    for session in &model.sessions {
        rows.push(serde_json::json!({
            "kind": "session",
            "label": format!("{} {}", if session.name == model.current { "🟢" } else { "📺" }, session.name),
            "session": session.name,
            "windows": session.windows,
            "current": session.name == model.current,
        }));
    }

    let mut layouts = model.layouts.clone();
    layouts.sort_by(|a, b| a.session.cmp(&b.session));
    for layout in &layouts {
        rows.push(serde_json::json!({
            "kind": "layout",
            "label": format!("🚀 {}", layout.session),
            "session": layout.session,
            "path": layout.path,
            "display": layout.display,
            "running": layout.running,
        }));
    }

    for project in &model.projects {
        rows.push(serde_json::json!({
            "kind": "project",
            "label": format!("🎬 {project}"),
            "project": project,
        }));
    }

    for dir in &model.zoxide {
        rows.push(serde_json::json!({
            "kind": "zoxide-dir",
            "label": format!("⭐ {}", dir.display),
            "path": dir.path,
            "display": dir.display,
            "has_local_config": dir.has_local_config,
        }));
    }

    for dir in &model.plain_repos {
        rows.push(serde_json::json!({
            "kind": "git-dir",
            "label": format!("📁 {}", dir.display),
            "path": dir.path,
            "display": dir.display,
            "has_local_config": dir.has_local_config,
        }));
    }

    rows
}

fn commands_json() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "binary": "muxpilot",
        "json_policy": {
            "success": "JSON commands emit only JSON on stdout",
            "errors": "JSON mode emits a machine-readable error object on stderr and exits nonzero",
            "format_flags": ["--json", "--format json", "-o json"]
        },
        "commands": [
            {"name": "doctor", "usage": "muxpilot doctor [--json]", "description": "check tmux/fzf availability and current tmux context"},
            {"name": "commands", "usage": "muxpilot commands [--json]", "description": "describe the CLI surface for agents"},
            {"name": "state", "usage": "muxpilot state [--json|--format json] [--capture]", "description": "print tmux sessions, windows, panes, and inferred agent state"},
            {"name": "agents", "usage": "muxpilot agents [--json|--format json] [--capture]", "description": "print only panes where an agent was detected"},
            {"name": "list", "usage": "muxpilot --list [--json|--format json]", "description": "print launchable menu rows without opening an interactive picker"},
            {"name": "switch", "usage": "muxpilot switch <session>", "description": "switch or attach to a tmux session"},
            {"name": "window", "usage": "muxpilot window <window-id>", "description": "select a tmux window by stable tmux window id"},
            {"name": "pane", "usage": "muxpilot pane <pane-id>", "description": "select a tmux pane by stable tmux pane id"},
            {"name": "panel", "usage": "muxpilot panel", "description": "run compact sidebar/panel mode"},
            {"name": "toggle-panel", "usage": "muxpilot toggle-panel", "description": "toggle the tmux sidebar pane"}
        ]
    })
}

fn print_commands(format: OutputFormat) -> Result<(), AppError> {
    match format {
        OutputFormat::Json => print_json(commands_json(), "muxpilot.commands.json"),
        OutputFormat::Human => {
            println!("{}", HELP_TEXT);
            Ok(())
        }
    }
}

fn run_doctor(format: OutputFormat) -> Result<ExitCode, AppError> {
    let tmux_version = command_output("tmux", &["-V"]);
    let fzf_version = command_output("fzf", &["--version"]);
    let inside_tmux = std::env::var_os("TMUX").is_some();
    let current_session = if inside_tmux {
        let session = tmux(&["display-message", "-p", "#{session_name}"]);
        (!session.is_empty()).then_some(session)
    } else {
        None
    };
    let ok = tmux_version.is_some();

    match format {
        OutputFormat::Json => print_json(
            serde_json::json!({
                "schema_version": 1,
                "ok": ok,
                "checks": {
                    "tmux": {
                        "available": tmux_version.is_some(),
                        "version": tmux_version,
                    },
                    "fzf": {
                        "available": fzf_version.is_some(),
                        "version": fzf_version,
                        "required": false,
                    },
                    "tmux_context": {
                        "inside_tmux": inside_tmux,
                        "current_session": current_session,
                    }
                }
            }),
            "muxpilot.doctor.json",
        )?,
        OutputFormat::Human => {
            println!("muxpilot doctor");
            println!(
                "tmux: {}",
                tmux_version.unwrap_or_else(|| "missing".to_string())
            );
            println!(
                "fzf: {}",
                fzf_version.unwrap_or_else(|| "missing (optional fallback only)".to_string())
            );
            println!("inside tmux: {inside_tmux}");
            if let Some(session) = current_session {
                println!("current session: {session}");
            }
        }
    }

    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Entry point. `args` are the CLI args after the program name.
const HELP_TEXT: &str = "muxpilot

  default                                native interactive picker
  --version, -V                          print the muxpilot version
  --fzf                                  use fzf picker fallback
  --list [--json|--format json]          print picker rows
  doctor [--json|--format json]          check tmux/fzf and tmux context
  commands [--json|--format json]        describe command surface
  state [--json|--format json] [--capture]
                                         print tmux session/window/pane state
  agents [--json|--format json] [--capture]
                                         print detected agent panes
  switch <session>                       switch/attach to session
  window <window-id>                     select tmux window
  pane <pane-id>                         select tmux pane
  panel                                  watch compact state for sidebar panes
  toggle-panel                           toggle a tmux sidebar pane running panel mode";

pub async fn run(args: Vec<String>) -> Result<ExitCode, AppError> {
    let format = output_format(&args)?;

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP_TEXT}");
        return Ok(ExitCode::SUCCESS);
    }

    if args
        .first()
        .is_some_and(|a| a == "-V" || a == "--version" || a == "version")
    {
        println!("muxpilot {}", env!("CARGO_PKG_VERSION"));
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "commands") {
        print_commands(format)?;
        return Ok(ExitCode::SUCCESS);
    }

    // Hidden maintainer command: render fake data at any scale to exercise
    // truncation/filtering and to drive reproducible screenshots/videos.
    if args.first().is_some_and(|a| a == "demo") {
        let count = args
            .iter()
            .position(|a| a == "--count" || a == "-n")
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200)
            .clamp(1, 100_000);
        return demo::run_demo(count);
    }

    if args.first().is_some_and(|a| a == "doctor") {
        return run_doctor(format);
    }

    if args.first().is_some_and(|a| a == "state" || a == "json") {
        let snapshot = tmux_snapshot_with_options(SnapshotOptions {
            capture_pane: args.iter().any(|a| a == "--capture"),
        });
        match if args.first().is_some_and(|a| a == "json") {
            OutputFormat::Json
        } else {
            format
        } {
            OutputFormat::Json => print_json(&snapshot, "muxpilot.state.json")?,
            OutputFormat::Human => println!("{}", render_snapshot_human(&snapshot)),
        }
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "agents") {
        let snapshot = tmux_snapshot_with_options(SnapshotOptions {
            capture_pane: args.iter().any(|a| a == "--capture"),
        });
        match format {
            OutputFormat::Json => {
                print_json(
                    serde_json::json!({
                    "schema_version": 1,
                    "backend": "tmux",
                    "agents": agent_rows(&snapshot),
                    }),
                    "muxpilot.agents.json",
                )?;
            }
            OutputFormat::Human => println!("{}", render_agents_human(&snapshot)),
        }
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "switch") {
        let Some(session) = args.get(1) else {
            eprintln!("Usage: muxpilot switch <session>");
            return Ok(ExitCode::FAILURE);
        };
        enter_session(session)?;
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "window") {
        let Some(window_id) = args.get(1) else {
            eprintln!("Usage: muxpilot window <window-id>");
            return Ok(ExitCode::FAILURE);
        };
        let Some((session, _)) = target_session_window(window_id) else {
            eprintln!("Unknown tmux window target: {window_id}");
            return Ok(ExitCode::FAILURE);
        };
        switch_tmux_window(&session, window_id)?;
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "pane") {
        let Some(pane_id) = args.get(1) else {
            eprintln!("Usage: muxpilot pane <pane-id>");
            return Ok(ExitCode::FAILURE);
        };
        let Some((session, window_id)) = target_session_window(pane_id) else {
            eprintln!("Unknown tmux pane target: {pane_id}");
            return Ok(ExitCode::FAILURE);
        };
        switch_tmux_pane(&session, &window_id, pane_id)?;
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().is_some_and(|a| a == "panel") {
        return run_watch_panel();
    }

    if args
        .first()
        .is_some_and(|a| a == "toggle-panel" || a == "sidebar" || a == "toggle-sidebar")
    {
        return toggle_panel();
    }

    let no_cache = args.iter().any(|a| a == "--no-cache");
    let model = build_model_cached(no_cache).await;
    let lines = build_menu_lines(&model);

    if lines.is_empty() {
        eprintln!("No sessions, projects, or directories found");
        return Ok(ExitCode::FAILURE);
    }

    let menu = lines.join("\n");

    // `--list`: print the resolved menu and exit (test/debug, no fzf).
    if args.iter().any(|a| a == "--list") {
        match format {
            OutputFormat::Json => print_json(
                serde_json::json!({
                    "schema_version": 1,
                    "rows": menu_rows_json(&model),
                }),
                "muxpilot.list.json",
            )?,
            OutputFormat::Human => println!("{menu}"),
        }
        return Ok(ExitCode::SUCCESS);
    }

    let selection = if args.iter().any(|a| a == "--fzf" || a == "fzf") {
        let choice = match select_with_fzf(&menu).await? {
            Some(c) => c,
            None => return Ok(ExitCode::SUCCESS), // user cancelled
        };
        match parse_selection(&choice, &home()) {
            Some(selection) => selection,
            None => {
                eprintln!("Invalid selection format");
                return Ok(ExitCode::FAILURE);
            }
        }
    } else {
        match select_native(&model).await? {
            Some(selection) => selection,
            None => return Ok(ExitCode::SUCCESS),
        }
    };
    execute(selection).await?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests;
