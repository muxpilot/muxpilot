use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, ErrorCode};
use serde::{Deserialize, Serialize};

pub(crate) const FIELD_SEP: &str = "::__TMUX_MENU__:";
pub(crate) const MENU_PANE_ROLE: &str = "muxpilot-panel";
const STATE_FORMAT: &str = "#{session_name}::__TMUX_MENU__:#{window_id}::__TMUX_MENU__:#{window_index}::__TMUX_MENU__:#{window_name}::__TMUX_MENU__:#{window_active}::__TMUX_MENU__:#{window_activity}::__TMUX_MENU__:#{pane_id}::__TMUX_MENU__:#{pane_active}::__TMUX_MENU__:#{pane_current_path}::__TMUX_MENU__:#{pane_current_command}::__TMUX_MENU__:#{pane_pid}::__TMUX_MENU__:#{pane_activity}::__TMUX_MENU__:#{@pane_agent}::__TMUX_MENU__:#{@pane_status}::__TMUX_MENU__:#{@pane_attention}::__TMUX_MENU__:#{@pane_wait_reason}::__TMUX_MENU__:#{@pane_role}::__TMUX_MENU__:#{@pane_model}";

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
    pub status: PaneAgentStatus,
    pub source: AgentStateSource,
    pub confidence: u8,
    pub attention: bool,
    pub wait_reason: String,
    /// The model the agent is running (e.g. `claude-opus-4-8`, `gpt-5.5`), when
    /// known — from the `@pane_model` hook option or the agent's process args.
    pub model: Option<String>,
    pub evidence: Vec<String>,
    /// The pane's captured screen changed since the previous snapshot (T3) — a
    /// secondary "moving now" signal. Note this hashes the whole capture, so a
    /// live spinner counts as a change; the primary working/idle decision keys
    /// off `status` and treats this only as a booster. Set from the capture path.
    pub is_active: bool,
    /// Epoch seconds when the captured screen last changed (T3). Only meaningful
    /// alongside `is_active`; it measures any screen delta, spinner frames included.
    pub last_change: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStateSource {
    Hook,
    Process,
    PaneCommand,
    CapturePane,
}

/// The state of a coding agent running in a pane. Formalizes what was previously
/// a free-text `status` string so glyphs, severity ordering, and the attention
/// decision all derive from one typed vocabulary instead of ad-hoc matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneAgentStatus {
    Working,
    WaitingInput,
    WaitingApprove,
    Idle,
    Error,
    RateLimited,
    Parked,
    Unknown,
}

impl PaneAgentStatus {
    /// Kebab-case wire/display string; matches the serde encoding so human
    /// output and `state --json` stay in lockstep.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::WaitingInput => "waiting-input",
            Self::WaitingApprove => "waiting-approve",
            Self::Idle => "idle",
            Self::Error => "error",
            Self::RateLimited => "rate-limited",
            Self::Parked => "parked",
            Self::Unknown => "unknown",
        }
    }

    /// Whether this state should pull the user's eye — the single source of the
    /// picker's attention aggregation. Only the higher-severity "needs you"
    /// states qualify.
    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::WaitingApprove | Self::WaitingInput | Self::Error | Self::RateLimited
        )
    }

    /// Short label for the narrow picker status column, where the full kebab
    /// name would truncate. The glyph carries the fine-grained state.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Working => "work",
            Self::WaitingApprove => "appr",
            Self::WaitingInput => "input",
            Self::Idle => "idle",
            Self::Error => "err",
            Self::RateLimited => "rate",
            Self::Parked => "park",
            Self::Unknown => "?",
        }
    }

    /// A distinct glyph per state for the picker's status column and the fleet
    /// summary (T4). Colour is applied separately by the theme; a live spinner
    /// replaces the Working glyph when content is actually moving.
    pub fn glyph(self) -> &'static str {
        // Geometric BMP glyphs only — each is one display cell so the status
        // column stays aligned (no emoji-variant width surprises).
        match self {
            Self::Working => "●",
            Self::WaitingApprove => "◆",
            Self::WaitingInput => "◇",
            Self::Idle => "○",
            Self::Error => "×",
            Self::RateLimited => "◔",
            Self::Parked => "▪",
            Self::Unknown => "·",
        }
    }

    /// Every state, in the order the help legend lists them (most-active first).
    pub const ALL: [Self; 8] = [
        Self::Working,
        Self::WaitingApprove,
        Self::WaitingInput,
        Self::Idle,
        Self::Error,
        Self::RateLimited,
        Self::Parked,
        Self::Unknown,
    ];

    /// One-line human meaning for the help legend. The colour word matches how
    /// the picker tints the glyph (accent = pulls the eye, green = live, dim =
    /// quiet), so the legend documents both shape and hue.
    pub fn description(self) -> &'static str {
        match self {
            Self::Working => "green · producing output right now",
            Self::WaitingApprove => "accent · wants your approval to proceed",
            Self::WaitingInput => "accent · waiting for your input",
            Self::Idle => "dim · at a prompt, nothing running",
            Self::Error => "accent · the agent hit an error",
            Self::RateLimited => "accent · throttled by the provider",
            Self::Parked => "dim · intentionally paused",
            Self::Unknown => "dim · a process is there but state is unclear",
        }
    }

    /// Ranking used to bubble the worst child state up to a session/window row
    /// (higher = more urgent).
    pub fn severity(self) -> u8 {
        match self {
            Self::WaitingApprove => 7,
            Self::WaitingInput => 6,
            Self::Error => 5,
            Self::RateLimited => 4,
            Self::Working => 3,
            Self::Idle => 2,
            Self::Parked => 1,
            Self::Unknown => 0,
        }
    }
}

impl std::fmt::Display for PaneAgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Map a free-text hook `@pane_status` value into the typed vocabulary.
/// Unrecognized values become `Unknown` (the raw string is preserved in the
/// pane's `evidence`), so a third-party hook cannot inject an invalid state.
fn parse_pane_status(raw: &str) -> PaneAgentStatus {
    match raw.trim().to_ascii_lowercase().as_str() {
        "working" | "busy" | "running" | "active" | "thinking" => PaneAgentStatus::Working,
        // A bare "waiting" from a hook is generic (waiting on the user); the
        // capture-pane path, which matches approval-style prompts specifically,
        // uses WaitingApprove instead.
        "waiting-input" | "waiting_input" | "input" | "waiting" => PaneAgentStatus::WaitingInput,
        "waiting-approve" | "waiting_approve" | "approve" | "approval" | "permission" => {
            PaneAgentStatus::WaitingApprove
        }
        "idle" | "done" | "ready" | "complete" | "completed" => PaneAgentStatus::Idle,
        "error" | "failed" | "failure" => PaneAgentStatus::Error,
        "rate-limited" | "rate_limited" | "ratelimited" | "rate-limit" => {
            PaneAgentStatus::RateLimited
        }
        "parked" => PaneAgentStatus::Parked,
        _ => PaneAgentStatus::Unknown,
    }
}

/// Resolve a pane's attention flag from the hook's explicit `@pane_attention`
/// value and the parsed status. An explicit value is authoritative in both
/// directions; an unset value falls back to whether the status needs attention.
fn attention_from_hook(raw_attention: &str, status: PaneAgentStatus) -> bool {
    match raw_attention.trim() {
        "" => status.needs_attention(),
        "clear" | "none" | "false" | "0" => false,
        _ => true,
    }
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
    // `cline` is listed before `claude`: a cline process often has a `claude`
    // child/model token, and detect_process_agent walks descendants — matching
    // the pane's own `cline` first keeps it from being mislabeled `claude`.
    ["cline", "claude", "codex", "opencode", "aider", "gemini", "amp"]
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

/// Extract a `--model X` / `--model=X` / `-m X` value from an agent's process
/// args — a fallback for when the `@pane_model` hook option isn't set.
fn model_from_args(pane_pid: Option<u32>, processes: &HashMap<u32, ProcessInfo>) -> Option<String> {
    let root = pane_pid?;
    for pid in std::iter::once(root).chain(descendants(root, processes)) {
        let Some(info) = processes.get(&pid) else {
            continue;
        };
        if detect_agent_name(&info.comm).is_none() && detect_agent_name(&info.args).is_none() {
            continue;
        }
        let tokens: Vec<&str> = info.args.split_whitespace().collect();
        for (i, tok) in tokens.iter().enumerate() {
            if let Some(v) = tok.strip_prefix("--model=").or_else(|| tok.strip_prefix("-m=")) {
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
            if (*tok == "--model" || *tok == "-m") && i + 1 < tokens.len() {
                let v = tokens[i + 1];
                if !v.starts_with('-') {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Env vars agents use to pin their model, in priority order. Read from the
/// running process — an honest best-effort guess, never a fabricated default.
const MODEL_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_MODEL",
    "CLAUDE_MODEL",
    "CLAUDE_CODE_MODEL",
    "OPENAI_MODEL",
    "CODEX_MODEL",
    "GEMINI_MODEL",
    "AIDER_MODEL",
];

/// Guess an agent's model from its process environment — a fallback for when the
/// `@pane_model` hook and `--model` arg are both absent (e.g. a plain
/// `claude --resume` where the user exported `ANTHROPIC_MODEL`). Walks the pane's
/// process tree like [`model_from_args`]. Linux-only (reads `/proc/<pid>/environ`);
/// a graceful no-op on mac/BSD, where the hook remains the way to report a model.
fn model_from_env(pane_pid: Option<u32>, processes: &HashMap<u32, ProcessInfo>) -> Option<String> {
    let root = pane_pid?;
    for pid in std::iter::once(root).chain(descendants(root, processes)) {
        let Some(info) = processes.get(&pid) else {
            continue;
        };
        if detect_agent_name(&info.comm).is_none() && detect_agent_name(&info.args).is_none() {
            continue;
        }
        if let Some(model) = read_proc_model_env(pid) {
            return Some(model);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_proc_model_env(pid: u32) -> Option<String> {
    let data = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    for entry in data.split(|&b| b == 0) {
        let Ok(text) = std::str::from_utf8(entry) else {
            continue;
        };
        let Some((key, value)) = text.split_once('=') else {
            continue;
        };
        if MODEL_ENV_KEYS.contains(&key) && !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_proc_model_env(_pid: u32) -> Option<String> {
    None
}

/// Guess an agent's model family from its on-screen status-line badge — the
/// last-resort fallback for when the `@pane_model` hook, `--model` arg, and model
/// env var are all absent, yet the agent renders its model right on screen (e.g.
/// Claude's `🤖 Op1M  780k left  87%` footer). Returns a `~`-prefixed family slug
/// (`~opus`) so the value reads as the low-confidence guess it is. The badge table
/// lives in data — the `agent` profile's `[[model]] from = "screen"` — so it is
/// tuned without a recompile; this delegates to that engine.
fn model_from_screen(text: &str) -> Option<String> {
    crate::profiles::registry().model_from_screen(text)
}

/// Resolve an agent's model, in strict priority order: the `@pane_model` hook
/// option is authoritative (the agent reports its own model), then the process
/// args, then a model pinned in the process environment, and finally — only when
/// all of those are absent — a low-confidence guess scraped from the agent's
/// on-screen status-line badge. A screen guess never overrides a value any higher
/// source produced.
fn resolve_model(
    hook_model: &str,
    pane_pid: Option<u32>,
    processes: &HashMap<u32, ProcessInfo>,
    screen: Option<&str>,
) -> Option<String> {
    let hook_model = hook_model.trim();
    if !hook_model.is_empty() {
        return Some(hook_model.to_string());
    }
    model_from_args(pane_pid, processes)
        .or_else(|| model_from_env(pane_pid, processes))
        .or_else(|| screen.and_then(model_from_screen))
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
        let status = parse_pane_status(hook_status);
        let mut evidence = vec!["@pane_agent".to_string()];
        if !hook_status.is_empty() {
            evidence.push(format!("@pane_status={hook_status}"));
        }
        return Some(AgentState {
            kind: hook_agent.to_string(),
            status,
            source: AgentStateSource::Hook,
            confidence: 100,
            // An explicit `@pane_attention` value wins in *both* directions:
            // "clear"/"none" force-suppresses (a hook saying "don't bug me"),
            // any other non-empty value force-raises. When unset, the status
            // implies it. Keeps the independent hook signal authoritative.
            attention: attention_from_hook(attention, status),
            wait_reason: wait_reason.to_string(),
            model: None,
            evidence,
            is_active: false,
            last_change: None,
        });
    }

    if let Some(kind) = detect_process_agent(pane_pid, processes) {
        return Some(AgentState {
            kind,
            // Present but state unobservable without a hook or capture.
            status: PaneAgentStatus::Unknown,
            source: AgentStateSource::Process,
            confidence: 85,
            attention: false,
            wait_reason: String::new(),
            model: None,
            evidence: vec!["process-tree".to_string()],
            is_active: false,
            last_change: None,
        });
    }

    detect_agent_name(command).map(|kind| AgentState {
        kind: kind.to_string(),
        status: PaneAgentStatus::Unknown,
        source: AgentStateSource::PaneCommand,
        confidence: 60,
        attention: false,
        wait_reason: String::new(),
        model: None,
        evidence: vec![format!("pane_current_command={command}")],
        is_active: false,
        last_change: None,
    })
}

// The agent screen-pattern tables (approval / working / wait-input needles, the
// spinner and animation glyph sets, and the working-line / ready-prompt shape
// tests) that used to live here are now data — the `agent` profile in
// `default-profiles.toml`, evaluated by `crate::profiles`. Adding or tuning a
// pattern no longer means editing Rust.

/// Classify a captured pane's screen into an agent status. Pure over the text
/// so it is unit-testable; only the tail (the live status area) is inspected.
/// Returns `(status, confidence, wait_reason)`. This is the fallback tier —
/// used only for hook-less agents — but it is what lets MuxPilot show
/// working/idle where hook-only competitors show nothing.
///
/// The pattern tables that drive this now live in data — the `agent` profile in
/// `default-profiles.toml`, evaluated by the [`crate::profiles`] engine — so
/// tuning a screen needle or adding a program's patterns no longer means a
/// recompile. This wrapper preserves the old signature and behavior exactly.
fn classify_capture(text: &str) -> (PaneAgentStatus, u8, String) {
    let c = crate::profiles::registry().classify_agent(text, crate::profiles::OutputSignals::default());
    (c.status, c.confidence, c.evidence)
}

// --- T3: content-based pane activity ---

/// One pane's last observed screen content (hashed) and when it last changed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PaneActivity {
    hash: u64,
    last_change: u64,
}

pub(crate) fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash_content(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Decide whether a pane is active by comparing its previous vs current content
/// hash. Returns `(is_active, updated record)`. A first observation is *not*
/// active (we have no prior to compare) but starts the clock.
fn resolve_activity(prev: Option<PaneActivity>, hash: u64, now: u64) -> (bool, PaneActivity) {
    match prev {
        Some(p) if p.hash == hash => (false, p),
        Some(_) => (true, PaneActivity { hash, last_change: now }),
        None => (false, PaneActivity { hash, last_change: now }),
    }
}

fn pane_activity_cache_path() -> String {
    std::env::var("MUXPILOT_ACTIVITY_CACHE").unwrap_or_else(|_| {
        format!(
            "{}/muxpilot-pane-activity.json",
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
        )
    })
}

fn load_pane_activity() -> HashMap<String, PaneActivity> {
    std::fs::read_to_string(pane_activity_cache_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist only the panes seen this pass, so the store self-prunes as panes die.
/// Writes to a temp file and renames into place so a concurrent picker never
/// reads a truncated store.
fn save_pane_activity(map: &HashMap<String, PaneActivity>) {
    let Ok(text) = serde_json::to_string(map) else {
        return;
    };
    let path = pane_activity_cache_path();
    let tmp = format!("{path}.{}.tmp", std::process::id());
    if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn capture_pane_text(pane_id: &str) -> String {
    tmux(&["capture-pane", "-pt", pane_id, "-S", "-80"])
}

/// Refine a pane from its captured screen and return the captured text, so the
/// caller can reuse it (e.g. to scrape a model badge) without a second
/// `capture-pane`. Returns `None` only when the pane captured empty.
fn infer_from_capture(
    pane_id: &str,
    pane: &mut TmuxPane,
    prev: &HashMap<String, PaneActivity>,
    current: &mut HashMap<String, PaneActivity>,
    now: u64,
) -> Option<String> {
    let text = capture_pane_text(pane_id);
    if text.is_empty() {
        return None;
    }
    let (is_active, record) = resolve_activity(prev.get(pane_id).copied(), hash_content(&text), now);
    current.insert(pane_id.to_string(), record);
    let last_change = Some(record.last_change);

    let lower = text.to_ascii_lowercase();
    let kind = pane
        .agent
        .as_ref()
        .map(|a| a.kind.clone())
        .or_else(|| detect_agent_name(&lower).map(ToOwned::to_owned));
    let Some(kind) = kind else {
        // No coding agent here — but the pane may still be a build / test /
        // deploy / shell process blocked on you (a Terraform apply prompt, an
        // ssh password, a compiler error). Surface only the "needs you" states,
        // so an idle or merely-working process never clutters the list.
        maybe_attach_process_state(pane, &text, is_active, last_change);
        return Some(text);
    };

    let (status, conf, wait_reason) = classify_capture(&text);

    match &mut pane.agent {
        Some(agent) => {
            // A hook is authoritative for status; capture only refines a non-hook
            // detection (upgrading Unknown into a real state). The content-activity
            // signal, however, applies to every source — including hooks — so it is
            // set unconditionally below.
            if !matches!(agent.source, AgentStateSource::Hook)
                && status != PaneAgentStatus::Unknown
            {
                agent.status = status;
                agent.attention = status.needs_attention();
                agent.wait_reason = wait_reason.clone();
                agent.confidence = agent.confidence.max(conf);
            }
            agent.is_active = is_active;
            agent.last_change = last_change;
            agent.evidence.push("capture-pane".to_string());
        }
        None => {
            pane.agent = Some(AgentState {
                kind,
                status,
                source: AgentStateSource::CapturePane,
                confidence: conf,
                attention: status.needs_attention(),
                wait_reason,
                model: None,
                evidence: vec!["capture-pane".to_string()],
                is_active,
                last_change,
            });
        }
    }
    Some(text)
}

/// Surface a non-agent pane's process state when it needs the user. Runs the
/// beyond-agents profiles (build / test / deploy / generic-shell) over the
/// captured screen; attaches a state only for `needs_attention()` statuses
/// (approval / input / error), so a plain idle or busy process stays invisible
/// and the picker's list keeps its focus on things that want you.
fn maybe_attach_process_state(
    pane: &mut TmuxPane,
    text: &str,
    is_active: bool,
    last_change: Option<u64>,
) {
    let signals = crate::profiles::OutputSignals::default();
    let Some((kind, class)) =
        crate::profiles::registry().classify_process(&pane.current_command, text, signals)
    else {
        return;
    };
    if !class.status.needs_attention() {
        return;
    }
    pane.agent = Some(AgentState {
        kind,
        status: class.status,
        source: AgentStateSource::CapturePane,
        confidence: class.confidence,
        attention: true,
        wait_reason: class.evidence,
        model: None,
        evidence: vec!["capture-pane".to_string(), "process-state".to_string()],
        is_active,
        last_change,
    });
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
        // T3: compare each pane's content against the previous snapshot to derive
        // an honest "active now" signal. `current` keeps only live panes, so the
        // on-disk store self-prunes when panes close.
        let prev_activity = if options.capture_pane {
            load_pane_activity()
        } else {
            HashMap::new()
        };
        let mut current_activity: HashMap<String, PaneActivity> = HashMap::new();
        let now = now_epoch();

        let mut session_order: Vec<String> = Vec::new();
        let mut sessions: BTreeMap<String, BTreeMap<String, TmuxWindow>> = BTreeMap::new();

        for line in raw.lines() {
            let parts: Vec<&str> = line.split(FIELD_SEP).collect();
            if parts.len() < 18 {
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
            // Capture every pane when enabled: besides refining known agents,
            // this keeps the screen-only detection path alive (an agent visible
            // on screen that process/command matching missed). infer_from_capture
            // returns early for panes with no agent signal, so the cost is a
            // capture-pane call per pane on open/refresh only.
            let captured = if options.capture_pane {
                let pane_id = pane.id.clone();
                infer_from_capture(
                    &pane_id,
                    &mut pane,
                    &prev_activity,
                    &mut current_activity,
                    now,
                )
            } else {
                None
            };
            // Resolve the model once the agent (if any) is finalized: the
            // @pane_model hook option wins, else process args, then the env, then
            // a low-confidence guess scraped from the captured screen badge.
            if let Some(agent) = pane.agent.as_mut() {
                agent.model = resolve_model(parts[17], pane_pid, &processes, captured.as_deref());
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

        if options.capture_pane {
            save_pane_activity(&current_activity);
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

#[cfg(test)]
mod pane_status_tests {
    use super::*;

    #[test]
    fn parse_maps_synonyms_and_unknown() {
        assert_eq!(parse_pane_status("busy"), PaneAgentStatus::Working);
        assert_eq!(parse_pane_status("  Working "), PaneAgentStatus::Working);
        assert_eq!(parse_pane_status("permission"), PaneAgentStatus::WaitingApprove);
        assert_eq!(parse_pane_status("waiting"), PaneAgentStatus::WaitingInput);
        assert_eq!(parse_pane_status("done"), PaneAgentStatus::Idle);
        assert_eq!(parse_pane_status("rate-limit"), PaneAgentStatus::RateLimited);
        assert_eq!(parse_pane_status(""), PaneAgentStatus::Unknown);
        assert_eq!(parse_pane_status("something-else"), PaneAgentStatus::Unknown);
    }

    #[test]
    fn needs_attention_matches_kebab_and_serialization() {
        for (status, kebab, attn) in [
            (PaneAgentStatus::Working, "working", false),
            (PaneAgentStatus::WaitingInput, "waiting-input", true),
            (PaneAgentStatus::WaitingApprove, "waiting-approve", true),
            (PaneAgentStatus::Idle, "idle", false),
            (PaneAgentStatus::Error, "error", true),
            (PaneAgentStatus::RateLimited, "rate-limited", true),
            (PaneAgentStatus::Parked, "parked", false),
            (PaneAgentStatus::Unknown, "unknown", false),
        ] {
            assert_eq!(status.as_str(), kebab);
            assert_eq!(status.to_string(), kebab);
            assert_eq!(serde_json::to_string(&status).unwrap(), format!("\"{kebab}\""));
            assert_eq!(status.needs_attention(), attn);
        }
    }

    #[test]
    fn severity_orders_attention_states_above_the_rest() {
        // The four attention-worthy states must outrank the calm ones so a
        // session row bubbles the state that needs the user.
        let attention = [
            PaneAgentStatus::WaitingApprove,
            PaneAgentStatus::WaitingInput,
            PaneAgentStatus::Error,
            PaneAgentStatus::RateLimited,
        ];
        let calm = [
            PaneAgentStatus::Working,
            PaneAgentStatus::Idle,
            PaneAgentStatus::Parked,
            PaneAgentStatus::Unknown,
        ];
        let min_attention = attention.iter().map(|s| s.severity()).min().unwrap();
        let max_calm = calm.iter().map(|s| s.severity()).max().unwrap();
        assert!(min_attention > max_calm);
        assert_eq!(PaneAgentStatus::WaitingApprove.severity(), 7);
    }

    #[test]
    fn classify_capture_reads_common_agent_screens() {
        use PaneAgentStatus::*;
        // Working: spinner and/or an interrupt hint.
        assert_eq!(classify_capture("⠹ Thinking… (esc to interrupt)").0, Working);
        assert_eq!(classify_capture("out\n· Running a tool (esc to stop)").0, Working);
        // Approval gate outranks a spinner rendered alongside it.
        assert_eq!(
            classify_capture("⠋ Do you want to proceed?\n❯ 1. Yes\n  2. No").0,
            WaitingApprove
        );
        // Free-form input prompt.
        assert_eq!(classify_capture("Type your answer (esc to cancel)").0, WaitingInput);
        // Idle: an empty ready-prompt box as the tail.
        assert_eq!(classify_capture("done.\n╭─────╮\n│ >   │\n╰─────╯").0, Idle);
        // Indeterminate scrollback (note: `>` inside a command is not a prompt).
        assert_eq!(classify_capture("ran: cat a > b.txt\nplain text").0, Unknown);
    }

    #[test]
    fn classify_capture_reads_real_claude_layout() {
        use PaneAgentStatus::*;
        // Claude working: the "Sketching…" status line sits ABOVE the input box
        // and status bar — 6 lines from the bottom — so the classifier must look
        // past them. Previously this whole screen read as Idle.
        let working = "\
… +17 lines (ctrl+o to expand)
✽ Sketching… (14m 0s · ↓ 46.4k tokens)
  Tip: Use /btw to ask a quick side question
────────────────────────────
❯
────────────────────────────
  Op1M  780k left  87%  3h58m
  -- INSERT -- bypass permissions on";
        assert_eq!(classify_capture(working).0, Working, "Sketching… is working");

        // Claude finished: same glyph family, but the line reads "Churned for" —
        // no ellipsis — with an empty prompt below. That is idle, not working.
        let done = "\
  Only the genuinely-unmerged branch was preserved.
✻ Churned for 2m 33s
────────────────────────────
❯
────────────────────────────
  Op1M  610k left  92%  17m
  -- INSERT -- bypass permissions on";
        assert_eq!(classify_capture(done).0, Idle, "Churned for = done/idle");

        // The word "approval" in the agent's *prose* must not read as a permission
        // gate — only an actual numbered/y-n prompt does.
        let prose = "\
  Tap a button on the \"Approval gate\" or \"Poll\" message and the webhook answers.
  Want me to wire the callback routing so buttons do per-action things?
✻ Churned for 18m 32s
────────────────────────────
❯ yes, wire the callback routing
────────────────────────────
  Op1M  760k left  86%";
        assert_ne!(
            classify_capture(prose).0,
            WaitingApprove,
            "prose mentioning approval is not a gate"
        );
    }

    #[test]
    fn classify_capture_confidence_tracks_status() {
        let (status, conf, reason) = classify_capture("Approve this action? (y/n)");
        assert_eq!((status, conf), (PaneAgentStatus::WaitingApprove, 80));
        assert!(status.needs_attention());
        assert!(!reason.is_empty());
        assert_eq!(classify_capture("⠋ working…").1, 70);
        assert!(!classify_capture("⠋ working…").0.needs_attention());
    }

    #[test]
    fn model_from_args_reads_agent_flags() {
        let mut procs: HashMap<u32, ProcessInfo> = HashMap::new();
        procs.insert(
            100,
            ProcessInfo {
                ppid: 1,
                comm: "node".to_string(),
                args: "node /usr/bin/claude --model opus-4.8 --foo".to_string(),
            },
        );
        assert_eq!(
            model_from_args(Some(100), &procs).as_deref(),
            Some("opus-4.8")
        );

        procs.insert(
            200,
            ProcessInfo {
                ppid: 1,
                comm: "codex".to_string(),
                args: "codex -m=gpt-5.5".to_string(),
            },
        );
        assert_eq!(
            model_from_args(Some(200), &procs).as_deref(),
            Some("gpt-5.5")
        );

        // A non-agent process yields nothing.
        procs.insert(
            300,
            ProcessInfo {
                ppid: 1,
                comm: "zsh".to_string(),
                args: "zsh -l".to_string(),
            },
        );
        assert_eq!(model_from_args(Some(300), &procs), None);

        // The hook option wins over args when present.
        assert_eq!(
            resolve_model("claude-opus-4-8", Some(100), &procs, None).as_deref(),
            Some("claude-opus-4-8")
        );
    }

    #[test]
    fn model_from_screen_reads_status_badge() {
        // Claude's on-screen footer badge → a `~`-prefixed family guess.
        let screen = "some prose\n🤖 Op1M 🔥  █░░░░░ 780k left  🔋 76%  ⏱ 2h56m\n-- INSERT --";
        assert_eq!(model_from_screen(screen).as_deref(), Some("~opus"));
        assert_eq!(
            model_from_screen("… Sonnet  120k left  40%").as_deref(),
            Some("~sonnet")
        );
        // A plain shell screen with no badge yields nothing.
        assert_eq!(model_from_screen("$ ls -la\ntotal 0\n$ "), None);

        // Screen scrape is strictly last: any higher source wins, and the guess
        // only fills in when hook + args + env are all absent.
        let mut procs: HashMap<u32, ProcessInfo> = HashMap::new();
        procs.insert(
            100,
            ProcessInfo {
                ppid: 1,
                comm: "node".to_string(),
                args: "node /usr/bin/claude --model opus-4.8".to_string(),
            },
        );
        // args present → screen ignored.
        assert_eq!(
            resolve_model("", Some(100), &procs, Some("🤖 Op1M 780k left")).as_deref(),
            Some("opus-4.8")
        );
        // no hook/args/env → screen guess fills in.
        let bare: HashMap<u32, ProcessInfo> = HashMap::new();
        assert_eq!(
            resolve_model("", Some(999), &bare, Some("🤖 Op1M 780k left")).as_deref(),
            Some("~opus")
        );
    }

    #[test]
    fn resolve_activity_flags_content_change_only() {
        let h1 = hash_content("screen A");
        let h2 = hash_content("screen B");
        assert_ne!(h1, h2);
        assert_eq!(hash_content("screen A"), h1, "hash is stable");

        // First observation: not active, clock starts now.
        let (active, rec) = resolve_activity(None, h1, 1000);
        assert!(!active);
        assert_eq!(rec.last_change, 1000);

        // Same content later: still not active, last_change preserved.
        let (active, rec) = resolve_activity(Some(rec), h1, 2000);
        assert!(!active);
        assert_eq!(rec.last_change, 1000);

        // Changed content: active, clock resets.
        let (active, rec) = resolve_activity(Some(rec), h2, 3000);
        assert!(active);
        assert_eq!(rec.last_change, 3000);
    }

    #[test]
    fn hook_attention_flag_is_authoritative_in_both_directions() {
        // Explicit "clear" suppresses even an attention-worthy status...
        assert!(!attention_from_hook("clear", PaneAgentStatus::Error));
        assert!(!attention_from_hook("none", PaneAgentStatus::WaitingApprove));
        // ...an explicit truthy value raises even a calm status...
        assert!(attention_from_hook("yes", PaneAgentStatus::Working));
        // ...and an unset flag falls back to the status implication.
        assert!(attention_from_hook("", PaneAgentStatus::WaitingApprove));
        assert!(!attention_from_hook("  ", PaneAgentStatus::Working));
    }
}
