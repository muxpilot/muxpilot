use std::collections::{BTreeMap, HashMap};

use crate::error::{AppError, ErrorCode};
use serde::Serialize;

pub(crate) const FIELD_SEP: &str = "::__TMUX_MENU__:";
pub(crate) const MENU_PANE_ROLE: &str = "muxpilot-panel";
const STATE_FORMAT: &str = "#{session_name}::__TMUX_MENU__:#{window_id}::__TMUX_MENU__:#{window_index}::__TMUX_MENU__:#{window_name}::__TMUX_MENU__:#{window_active}::__TMUX_MENU__:#{window_activity}::__TMUX_MENU__:#{pane_id}::__TMUX_MENU__:#{pane_active}::__TMUX_MENU__:#{pane_current_path}::__TMUX_MENU__:#{pane_current_command}::__TMUX_MENU__:#{pane_pid}::__TMUX_MENU__:#{pane_activity}::__TMUX_MENU__:#{@pane_agent}::__TMUX_MENU__:#{@pane_status}::__TMUX_MENU__:#{@pane_attention}::__TMUX_MENU__:#{@pane_wait_reason}::__TMUX_MENU__:#{@pane_role}";

#[derive(Debug, Clone, Serialize)]
pub struct TmuxSnapshot {
    pub schema_version: u32,
    pub source: &'static str,
    pub backend: &'static str,
    pub current_session: String,
    pub current_window_id: String,
    pub current_pane_id: String,
    pub sessions: Vec<TmuxSession>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmuxSession {
    pub name: String,
    pub windows: Vec<TmuxWindow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmuxWindow {
    pub id: String,
    pub index: u32,
    pub name: String,
    pub active: bool,
    pub last_activity: Option<u64>,
    pub panes: Vec<TmuxPane>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmuxPane {
    pub id: String,
    pub active: bool,
    pub path: String,
    pub current_command: String,
    pub pid: Option<u32>,
    pub last_activity: Option<u64>,
    pub role: String,
    pub agent: Option<AgentState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub kind: String,
    pub status: String,
    pub source: AgentStateSource,
    pub confidence: u8,
    pub attention: bool,
    pub wait_reason: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStateSource {
    Hook,
    Process,
    PaneCommand,
    CapturePane,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotOptions {
    pub capture_pane: bool,
}

trait MuxBackend {
    fn snapshot(&self, options: SnapshotOptions) -> TmuxSnapshot;
}

struct TmuxBackend;

#[derive(Debug, Clone)]
struct ProcessInfo {
    ppid: u32,
    comm: String,
    args: String,
}

pub(crate) fn tmux(args: &[&str]) -> String {
    std::process::Command::new("tmux")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub(crate) fn tmux_checked(args: &[&str], operation: &str) -> Result<(), AppError> {
    let output = std::process::Command::new("tmux")
        .args(args)
        .output()
        .map_err(|e| {
            AppError::new(
                ErrorCode::MissingIntegration,
                format!("failed to run tmux for {operation}: {e}"),
            )
            .with_source(e)
            .op(operation)
        })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(AppError::new(
        ErrorCode::ProcessNonzeroExit,
        if stderr.is_empty() {
            format!("tmux command failed for {operation}")
        } else {
            format!("tmux command failed for {operation}: {stderr}")
        },
    )
    .op(operation))
}

pub(crate) fn command_output(command: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
}

fn parse_bool(s: &str) -> bool {
    matches!(s, "1" | "true" | "yes" | "on")
}

fn parse_tmux_timestamp(s: &str) -> Option<u64> {
    let timestamp = s.trim().parse::<u64>().ok()?;
    (timestamp > 0).then_some(timestamp)
}

fn parse_ps() -> HashMap<u32, ProcessInfo> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm=,args="])
        .output()
        .ok()
        .filter(|o| o.status.success());
    let Some(output) = output else {
        return HashMap::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.trim_start().splitn(4, char::is_whitespace);
            let pid = parts.next()?.parse::<u32>().ok()?;
            let ppid = parts.next()?.trim_start().parse::<u32>().ok()?;
            let comm = parts.next()?.trim_start().to_string();
            let args = parts.next().unwrap_or("").trim_start().to_string();
            Some((pid, ProcessInfo { ppid, comm, args }))
        })
        .collect()
}

fn descendants(root: u32, processes: &HashMap<u32, ProcessInfo>) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, info) in processes {
        children.entry(info.ppid).or_default().push(*pid);
    }

    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if pid != root {
            out.push(pid);
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids);
        }
    }
    out
}

fn command_basename(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

fn detect_agent_name(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    ["claude", "codex", "opencode", "aider", "gemini", "amp"]
        .into_iter()
        .find(|&name| {
            lower
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .any(|token| token == name || token.strip_suffix(".js") == Some(name))
        })
}

fn detect_process_agent(
    pane_pid: Option<u32>,
    processes: &HashMap<u32, ProcessInfo>,
) -> Option<String> {
    let root = pane_pid?;
    for pid in std::iter::once(root).chain(descendants(root, processes).into_iter()) {
        let Some(info) = processes.get(&pid) else {
            continue;
        };
        let comm = command_basename(&info.comm);
        if let Some(agent) = detect_agent_name(comm).or_else(|| detect_agent_name(&info.args)) {
            return Some(agent.to_string());
        }
    }
    None
}

fn infer_agent(
    hook_agent: &str,
    hook_status: &str,
    attention: &str,
    wait_reason: &str,
    command: &str,
    pane_pid: Option<u32>,
    processes: &HashMap<u32, ProcessInfo>,
) -> Option<AgentState> {
    if !hook_agent.is_empty() {
        return Some(AgentState {
            kind: hook_agent.to_string(),
            status: if hook_status.is_empty() {
                "detected".to_string()
            } else {
                hook_status.to_string()
            },
            source: AgentStateSource::Hook,
            confidence: 100,
            attention: !attention.is_empty() && attention != "clear",
            wait_reason: wait_reason.to_string(),
            evidence: vec!["@pane_agent".to_string()],
        });
    }

    if let Some(kind) = detect_process_agent(pane_pid, processes) {
        return Some(AgentState {
            kind,
            status: "detected".to_string(),
            source: AgentStateSource::Process,
            confidence: 85,
            attention: false,
            wait_reason: String::new(),
            evidence: vec!["process-tree".to_string()],
        });
    }

    detect_agent_name(command).map(|kind| AgentState {
        kind: kind.to_string(),
        status: "detected".to_string(),
        source: AgentStateSource::PaneCommand,
        confidence: 60,
        attention: false,
        wait_reason: String::new(),
        evidence: vec![format!("pane_current_command={command}")],
    })
}

fn capture_pane_text(pane_id: &str) -> String {
    tmux(&["capture-pane", "-pt", pane_id, "-S", "-80"])
}

fn infer_from_capture(pane_id: &str, pane: &mut TmuxPane) {
    let text = capture_pane_text(pane_id);
    if text.is_empty() {
        return;
    }
    let lower = text.to_ascii_lowercase();
    let kind = pane
        .agent
        .as_ref()
        .map(|a| a.kind.clone())
        .or_else(|| detect_agent_name(&lower).map(ToOwned::to_owned));
    let Some(kind) = kind else {
        return;
    };

    let waiting = [
        "continue?",
        "do you want to",
        "proceed?",
        "approve",
        "approval",
        "press enter",
        "y/n",
        "yes/no",
        "❯ 1.",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    let status = if waiting { "waiting" } else { "visible" }.to_string();
    let wait_reason = if waiting {
        "screen prompt or approval text".to_string()
    } else {
        String::new()
    };

    match &mut pane.agent {
        Some(agent) if !matches!(agent.source, AgentStateSource::Hook) => {
            if waiting {
                agent.status = status;
                agent.attention = true;
                agent.wait_reason = wait_reason;
            }
            agent.confidence = agent.confidence.max(90);
            agent.evidence.push("capture-pane".to_string());
        }
        None => {
            pane.agent = Some(AgentState {
                kind,
                status,
                source: AgentStateSource::CapturePane,
                confidence: if waiting { 80 } else { 50 },
                attention: waiting,
                wait_reason,
                evidence: vec!["capture-pane".to_string()],
            });
        }
        _ => {}
    }
}

pub fn tmux_snapshot() -> TmuxSnapshot {
    TmuxBackend.snapshot(SnapshotOptions::default())
}

pub fn tmux_snapshot_with_options(options: SnapshotOptions) -> TmuxSnapshot {
    TmuxBackend.snapshot(options)
}

impl MuxBackend for TmuxBackend {
    fn snapshot(&self, options: SnapshotOptions) -> TmuxSnapshot {
        let current_session = tmux(&["display-message", "-p", "#{session_name}"]);
        let current_window_id = tmux(&["display-message", "-p", "#{window_id}"]);
        let current_pane_id = tmux(&["display-message", "-p", "#{pane_id}"]);
        let raw = tmux(&["list-panes", "-a", "-F", STATE_FORMAT]);
        let processes = parse_ps();

        let mut session_order: Vec<String> = Vec::new();
        let mut sessions: BTreeMap<String, BTreeMap<String, TmuxWindow>> = BTreeMap::new();

        for line in raw.lines() {
            let parts: Vec<&str> = line.split(FIELD_SEP).collect();
            if parts.len() < 17 {
                continue;
            }
            let session_name = parts[0].to_string();
            let window_id = parts[1].to_string();
            if !sessions.contains_key(&session_name) {
                session_order.push(session_name.clone());
            }
            let pane_pid = parts[10].parse::<u32>().ok();
            let mut pane = TmuxPane {
                id: parts[6].to_string(),
                active: parse_bool(parts[7]),
                path: parts[8].to_string(),
                current_command: parts[9].to_string(),
                pid: pane_pid,
                last_activity: parse_tmux_timestamp(parts[11]),
                agent: infer_agent(
                    parts[12], parts[13], parts[14], parts[15], parts[9], pane_pid, &processes,
                ),
                role: parts[16].to_string(),
            };
            if options.capture_pane {
                let pane_id = pane.id.clone();
                infer_from_capture(&pane_id, &mut pane);
            }
            let window = sessions
                .entry(session_name)
                .or_default()
                .entry(window_id.clone())
                .or_insert_with(|| TmuxWindow {
                    id: window_id,
                    index: parts[2].parse::<u32>().unwrap_or(0),
                    name: parts[3].to_string(),
                    active: parse_bool(parts[4]),
                    last_activity: parse_tmux_timestamp(parts[5]),
                    panes: Vec::new(),
                });
            window.panes.push(pane);
        }

        let sessions = session_order
            .into_iter()
            .filter_map(|name| {
                let windows = sessions.remove(&name)?;
                let mut windows: Vec<TmuxWindow> = windows.into_values().collect();
                windows.sort_by_key(|w| w.index);
                Some(TmuxSession { name, windows })
            })
            .collect();

        TmuxSnapshot {
            schema_version: 1,
            source: "tmux",
            backend: "tmux",
            current_session,
            current_window_id,
            current_pane_id,
            sessions,
        }
    }
}

pub fn render_snapshot_human(snapshot: &TmuxSnapshot) -> String {
    let mut out = Vec::new();
    out.push(format!(
        "tmux: current session={} window={} pane={}",
        snapshot.current_session, snapshot.current_window_id, snapshot.current_pane_id
    ));
    for session in &snapshot.sessions {
        out.push(format!("session {}", session.name));
        for window in &session.windows {
            let active = if window.active { "*" } else { " " };
            out.push(format!(
                "  {active} window {}:{} {}",
                window.index, window.id, window.name
            ));
            for pane in &window.panes {
                let marker = if pane.active { ">" } else { " " };
                let agent = pane
                    .agent
                    .as_ref()
                    .map(|a| {
                        let attention = if a.attention { " !" } else { "" };
                        format!(" [{}:{} via {:?}{}]", a.kind, a.status, a.source, attention)
                    })
                    .unwrap_or_default();
                let role = if pane.role.is_empty() {
                    String::new()
                } else {
                    format!(" role={}", pane.role)
                };
                out.push(format!(
                    "    {marker} pane {} pid={} cmd={} cwd={}{}{}",
                    pane.id,
                    pane.pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    pane.current_command,
                    pane.path,
                    role,
                    agent
                ));
            }
        }
    }
    out.join("\n")
}
