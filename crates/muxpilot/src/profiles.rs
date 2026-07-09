//! A declarative, data-driven engine for classifying a tmux pane's screen into
//! the fixed [`PaneAgentStatus`] vocabulary.
//!
//! The churny part of state detection — *which screen patterns mean which
//! state*, for coding agents and for other long-running processes — lives in
//! TOML **profiles**, not in Rust. Defaults are compiled into the binary
//! ([`DEFAULT_PROFILES`]); an optional `~/.config/muxpilot/profiles.toml`
//! overrides any profile by `id`. The enum, its severity order, and the
//! confidence ladder stay owned by Rust: a rule's `status` must resolve to a
//! real variant or the profile is rejected at load (logged, never fatal), so no
//! data change can invent a state or break rendering.
//!
//! The engine is a pure classifier — `snapshot in → status/evidence out`, no
//! side effects, safe to evaluate across every pane. Profiles are data, not
//! code: they cannot exec, send keys, or touch the network.
//!
//! See the `agent-state-dsl` research note for the full design.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::snapshot::PaneAgentStatus;

/// The built-in profiles, compiled in so a zero-config install still classifies.
const DEFAULT_PROFILES: &str = include_str!("default-profiles.toml");

/// The outcome of classifying a pane: a typed status, the confidence that backs
/// it (the existing ladder numbers), and a human `evidence` string.
#[derive(Debug, Clone)]
pub struct Classification {
    pub status: PaneAgentStatus,
    pub confidence: u8,
    pub evidence: String,
}

// --------------------------------------------------------------------------
// Raw TOML shapes (deserialized as-authored, then compiled/validated).
// Unknown fields are ignored, so a newer profile file degrades gracefully.
// --------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    engine: RawEngine,
    #[serde(default)]
    spinner_sets: HashMap<String, String>,
    #[serde(default, rename = "profile")]
    profiles: Vec<RawProfile>,
}

#[derive(Debug, Default, Deserialize)]
struct RawEngine {
    #[serde(default)]
    limits: RawLimits,
}

#[derive(Debug, Deserialize)]
struct RawLimits {
    #[serde(default = "d_max_rules")]
    max_rules_per_profile: usize,
    #[serde(default = "d_max_profiles")]
    max_profiles: usize,
    #[serde(default = "d_max_tail")]
    max_tail_lines: usize,
}

impl Default for RawLimits {
    fn default() -> Self {
        Self {
            max_rules_per_profile: d_max_rules(),
            max_profiles: d_max_profiles(),
            max_tail_lines: d_max_tail(),
        }
    }
}

fn d_max_rules() -> usize {
    100
}
fn d_max_profiles() -> usize {
    500
}
fn d_max_tail() -> usize {
    120
}
fn d_tail() -> usize {
    10
}
fn d_priority() -> i32 {
    50
}
fn d_unknown_conf() -> u8 {
    50
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default = "d_priority")]
    priority: i32,
    #[serde(default = "d_tail")]
    tail_lines: usize,
    #[serde(default = "d_unknown_conf")]
    unknown_confidence: u8,
    #[serde(default)]
    identity: RawIdentity,
    #[serde(default, rename = "rule")]
    rules: Vec<RawRule>,
    #[serde(default, rename = "model")]
    model: Vec<RawModel>,
}

#[derive(Debug, Default, Deserialize)]
struct RawIdentity {
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    screen_tokens: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    status: String,
    confidence: u8,
    #[serde(default)]
    evidence: String,
    when: RawWhen,
}

/// A boolean rule body. Empty groups are vacuously satisfied; a `when` matches
/// iff its (optional) shorthand condition holds AND every `all` holds AND at
/// least one `any` holds (when `any` is non-empty) AND no `not` holds.
#[derive(Debug, Default, Deserialize)]
struct RawWhen {
    #[serde(default)]
    all: Vec<RawCond>,
    #[serde(default)]
    any: Vec<RawCond>,
    #[serde(default)]
    not: Vec<RawCond>,
    #[serde(flatten)]
    shorthand: RawCond,
}

/// A single condition. A condition is true iff every sub-field it sets is true;
/// a list field is true if any of its needles match. A condition with no fields
/// set is false, so an empty rule never fires.
#[derive(Debug, Default, Deserialize)]
struct RawCond {
    #[serde(default)]
    screen_any_substring_i: Vec<String>,
    #[serde(default)]
    screen_any_substring: Vec<String>,
    screen_spinner: Option<String>,
    screen_working_line: Option<bool>,
    screen_ready_prompt: Option<bool>,
    last_line_substring_i: Option<String>,
    unchanged_for_ms: Option<u64>,
    changed_within_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawModel {
    from: String,
    #[serde(default)]
    badges: Vec<(String, String)>,
}

// --------------------------------------------------------------------------
// Compiled (validated) shapes.
// --------------------------------------------------------------------------

#[derive(Debug)]
pub struct Registry {
    profiles: Vec<Profile>,
    /// Union of the `anim` + `braille` glyph sets, used by `screen_working_line`.
    working_line_glyphs: Vec<char>,
    agent_index: Option<usize>,
}

#[derive(Debug)]
struct Profile {
    id: String,
    kind: String,
    priority: i32,
    tail_lines: usize,
    unknown_confidence: u8,
    commands: Vec<String>,
    screen_tokens: Vec<String>,
    rules: Vec<Rule>,
    model: Vec<ModelExtract>,
}

#[derive(Debug)]
struct Rule {
    status: PaneAgentStatus,
    confidence: u8,
    evidence: String,
    when: When,
}

#[derive(Debug)]
struct When {
    all: Vec<Cond>,
    any: Vec<Cond>,
    not: Vec<Cond>,
    shorthand: Option<Cond>,
}

#[derive(Debug)]
struct Cond {
    substr_i: Vec<String>,
    substr: Vec<String>,
    spinner: Option<Vec<char>>,
    working_line: bool,
    ready_prompt: bool,
    last_line_i: Option<String>,
    unchanged_for_ms: Option<u64>,
    changed_within_ms: Option<u64>,
}

#[derive(Debug)]
struct ModelExtract {
    badges: Vec<(String, String)>,
}

/// Signals a rule can match against beyond the raw screen — the content-delta
/// timings the snapshot already computes. All optional; absent = unmatched.
#[derive(Debug, Default, Clone, Copy)]
pub struct OutputSignals {
    pub unchanged_for_ms: Option<u64>,
    pub changed_within_ms: Option<u64>,
}

/// A prepared view over a pane's captured screen: the last `n` non-empty lines,
/// their lowercased join, and the single last non-empty line (lowercased).
struct ScreenView<'a> {
    lines: Vec<&'a str>,
    joined: String,
    lower: String,
    last_line_lower: String,
}

impl<'a> ScreenView<'a> {
    fn new(text: &'a str, tail_lines: usize) -> Self {
        // Bottom-up like the old classifier, capped so deep scrollback prose
        // can't false-match; then restored to natural order for joins.
        let mut rev: Vec<&str> = text
            .lines()
            .rev()
            .filter(|l| !l.trim().is_empty())
            .take(tail_lines)
            .collect();
        let last_line_lower = rev.first().map(|l| l.to_ascii_lowercase()).unwrap_or_default();
        rev.reverse();
        let joined = rev.join("\n");
        let lower = joined.to_ascii_lowercase();
        Self { lines: rev, joined, lower, last_line_lower }
    }
}

impl Cond {
    fn matches(&self, view: &ScreenView, out: &OutputSignals, glyphs: &[char]) -> bool {
        let mut any_field = false;
        if !self.substr_i.is_empty() {
            any_field = true;
            if !self.substr_i.iter().any(|n| view.lower.contains(n.as_str())) {
                return false;
            }
        }
        if !self.substr.is_empty() {
            any_field = true;
            if !self.substr.iter().any(|n| view.joined.contains(n.as_str())) {
                return false;
            }
        }
        if let Some(set) = &self.spinner {
            any_field = true;
            if !view.joined.chars().any(|c| set.contains(&c)) {
                return false;
            }
        }
        if self.working_line {
            any_field = true;
            if !view.lines.iter().any(|l| is_working_line(l, glyphs)) {
                return false;
            }
        }
        if self.ready_prompt {
            any_field = true;
            if !view.lines.iter().any(|l| is_ready_prompt(l)) {
                return false;
            }
        }
        if let Some(needle) = &self.last_line_i {
            any_field = true;
            if !view.last_line_lower.contains(needle.as_str()) {
                return false;
            }
        }
        if let Some(limit) = self.unchanged_for_ms {
            any_field = true;
            // Absent timing can't confirm "unchanged for N" — treat as no match.
            if out.unchanged_for_ms.is_none_or(|v| v < limit) {
                return false;
            }
        }
        if let Some(limit) = self.changed_within_ms {
            any_field = true;
            if out.changed_within_ms.is_none_or(|v| v > limit) {
                return false;
            }
        }
        any_field
    }
}

impl When {
    fn matches(&self, view: &ScreenView, out: &OutputSignals, glyphs: &[char]) -> bool {
        if let Some(c) = &self.shorthand {
            if !c.matches(view, out, glyphs) {
                return false;
            }
        }
        if !self.all.iter().all(|c| c.matches(view, out, glyphs)) {
            return false;
        }
        if !self.any.is_empty() && !self.any.iter().any(|c| c.matches(view, out, glyphs)) {
            return false;
        }
        if self.not.iter().any(|c| c.matches(view, out, glyphs)) {
            return false;
        }
        true
    }
}

/// Whether a line is an agent's in-progress status line — a spinner/anim glyph
/// leading a gerund that trails off in an ellipsis (`✽ Sketching…`). The
/// ellipsis distinguishes it from the finished line (`✻ Churned for 2m`).
fn is_working_line(line: &str, glyphs: &[char]) -> bool {
    let trimmed = line.trim_start();
    let leads = trimmed.chars().next().is_some_and(|c| glyphs.contains(&c));
    leads && (line.contains('…') || line.contains("..."))
}

/// Whether a line is an empty ready-prompt (`>` / `❯`) once box-drawing and
/// whitespace are stripped — the shape an idle agent's input box takes.
fn is_ready_prompt(line: &str) -> bool {
    let core: String = line
        .chars()
        .filter(|c| !c.is_whitespace() && !"│|╭╮╰╯─╌┆┊>❯".contains(*c))
        .collect();
    core.is_empty() && (line.contains('>') || line.contains('❯'))
}

/// Confidence bucket, mirroring the existing source ladder. Used as the primary
/// tie-break after severity so a conf-80 approval can't be hidden by a conf-78
/// spinner within the same tier.
fn bucket(conf: u8) -> u8 {
    match conf {
        85..=99 => 4,
        70..=84 => 3,
        55..=69 => 2,
        1..=54 => 1,
        0 => 0,
        _ => 5, // 100 (and any out-of-range value) is authoritative
    }
}

impl Profile {
    /// Evaluate every rule; return the winning classification, or Unknown at the
    /// profile's fallback confidence when nothing matched. Winner key:
    /// `severity() tier → confidence bucket → confidence → rule order`.
    fn classify(&self, view: &ScreenView, out: &OutputSignals, glyphs: &[char]) -> Classification {
        let winner = self
            .rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.when.matches(view, out, glyphs))
            .max_by(|(ai, a), (bi, b)| {
                a.status
                    .severity()
                    .cmp(&b.status.severity())
                    .then(bucket(a.confidence).cmp(&bucket(b.confidence)))
                    .then(a.confidence.cmp(&b.confidence))
                    // Lower rule index wins ties, so reverse the index compare.
                    .then(bi.cmp(ai))
            });
        match winner {
            Some((_, r)) => Classification {
                status: r.status,
                confidence: r.confidence,
                evidence: r.evidence.clone(),
            },
            None => Classification {
                status: PaneAgentStatus::Unknown,
                confidence: self.unknown_confidence,
                evidence: String::new(),
            },
        }
    }

    fn identifies(&self, basename: &str, screen_lower: &str) -> bool {
        self.commands.iter().any(|c| c == basename)
            || self
                .screen_tokens
                .iter()
                .any(|t| screen_lower.contains(t.as_str()))
    }
}

impl Registry {
    /// Classify an already-identified agent pane from its screen — the exact
    /// replacement for the old hardcoded `classify_capture`.
    pub fn classify_agent(&self, text: &str, out: OutputSignals) -> Classification {
        let Some(profile) = self.agent_index.and_then(|i| self.profiles.get(i)) else {
            return Classification {
                status: PaneAgentStatus::Unknown,
                confidence: 50,
                evidence: String::new(),
            };
        };
        let view = ScreenView::new(text, profile.tail_lines);
        profile.classify(&view, &out, &self.working_line_glyphs)
    }

    /// Guess an agent's model family from its on-screen status-line badge — the
    /// lowest-confidence model source (below hook/argv/env). Returns a
    /// `~`-prefixed family (`~opus`) so it reads as the guess it is.
    pub fn model_from_screen(&self, text: &str) -> Option<String> {
        let profile = self.agent_index.and_then(|i| self.profiles.get(i))?;
        // A wider window than classification: the badge often sits in the footer
        // a few rows below the live status line.
        let view = ScreenView::new(text, profile.tail_lines.max(12));
        for extract in &profile.model {
            if let Some((_, family)) = extract
                .badges
                .iter()
                .find(|(needle, _)| view.lower.contains(needle.as_str()))
            {
                return Some(family.clone());
            }
        }
        None
    }

    /// Classify a non-agent pane by its foreground command / screen tells.
    /// Returns `(profile kind, classification)` for the winning profile, or
    /// `None` when no profile identifies the pane or none of its rules fire.
    /// Used to surface build / test / deploy / shell states beyond agents.
    pub fn classify_process(
        &self,
        command: &str,
        text: &str,
        out: OutputSignals,
    ) -> Option<(String, Classification)> {
        let basename = command.rsplit('/').next().unwrap_or(command);
        let screen_lower = text.to_ascii_lowercase();
        let mut best: Option<(&Profile, Classification)> = None;
        for profile in &self.profiles {
            if profile.agent_role() || !profile.identifies(basename, &screen_lower) {
                continue;
            }
            let view = ScreenView::new(text, profile.tail_lines);
            let c = profile.classify(&view, &out, &self.working_line_glyphs);
            if c.status == PaneAgentStatus::Unknown {
                continue;
            }
            let better = match &best {
                None => true,
                Some((bp, bc)) => rank_key(c.status, c.confidence, profile.priority)
                    > rank_key(bc.status, bc.confidence, bp.priority),
            };
            if better {
                best = Some((profile, c));
            }
        }
        best.map(|(p, c)| (p.kind.clone(), c))
    }
}

impl Profile {
    fn agent_role(&self) -> bool {
        self.kind == "agent"
    }
}

/// Sortable key for cross-profile winner selection: severity, then confidence
/// bucket, then confidence, then profile priority (specific beats fallback).
fn rank_key(status: PaneAgentStatus, conf: u8, priority: i32) -> (u8, u8, u8, i32) {
    (status.severity(), bucket(conf), conf, priority)
}

// --------------------------------------------------------------------------
// Loading, compilation, validation.
// --------------------------------------------------------------------------

fn parse_status(raw: &str) -> Option<PaneAgentStatus> {
    Some(match raw {
        "Working" => PaneAgentStatus::Working,
        "WaitingInput" => PaneAgentStatus::WaitingInput,
        "WaitingApprove" => PaneAgentStatus::WaitingApprove,
        "Idle" => PaneAgentStatus::Idle,
        "Error" => PaneAgentStatus::Error,
        "RateLimited" => PaneAgentStatus::RateLimited,
        "Parked" => PaneAgentStatus::Parked,
        "Unknown" => PaneAgentStatus::Unknown,
        _ => return None,
    })
}

fn compile_cond(raw: RawCond, spinner_sets: &HashMap<String, Vec<char>>) -> Result<Cond, String> {
    let spinner = match raw.screen_spinner {
        Some(name) => Some(
            spinner_sets
                .get(&name)
                .cloned()
                .ok_or_else(|| format!("unknown spinner set `{name}`"))?,
        ),
        None => None,
    };
    Ok(Cond {
        substr_i: raw
            .screen_any_substring_i
            .into_iter()
            .map(|s| s.to_ascii_lowercase())
            .collect(),
        substr: raw.screen_any_substring,
        spinner,
        working_line: raw.screen_working_line.unwrap_or(false),
        ready_prompt: raw.screen_ready_prompt.unwrap_or(false),
        last_line_i: raw.last_line_substring_i.map(|s| s.to_ascii_lowercase()),
        unchanged_for_ms: raw.unchanged_for_ms,
        changed_within_ms: raw.changed_within_ms,
    })
}

fn cond_is_empty(c: &RawCond) -> bool {
    c.screen_any_substring_i.is_empty()
        && c.screen_any_substring.is_empty()
        && c.screen_spinner.is_none()
        && c.screen_working_line.is_none()
        && c.screen_ready_prompt.is_none()
        && c.last_line_substring_i.is_none()
        && c.unchanged_for_ms.is_none()
        && c.changed_within_ms.is_none()
}

fn compile_when(raw: RawWhen, sets: &HashMap<String, Vec<char>>) -> Result<When, String> {
    let shorthand = if cond_is_empty(&raw.shorthand) {
        None
    } else {
        Some(compile_cond(raw.shorthand, sets)?)
    };
    let all = raw
        .all
        .into_iter()
        .map(|c| compile_cond(c, sets))
        .collect::<Result<_, _>>()?;
    let any = raw
        .any
        .into_iter()
        .map(|c| compile_cond(c, sets))
        .collect::<Result<_, _>>()?;
    let not = raw
        .not
        .into_iter()
        .map(|c| compile_cond(c, sets))
        .collect::<Result<_, _>>()?;
    Ok(When { all, any, not, shorthand })
}

impl Registry {
    fn compile(config: RawConfig) -> Self {
        let sets: HashMap<String, Vec<char>> = config
            .spinner_sets
            .iter()
            .map(|(k, v)| (k.clone(), v.chars().collect()))
            .collect();
        // `screen_working_line` keys off the union of the anim + braille sets.
        let mut working_line_glyphs: Vec<char> = Vec::new();
        for name in ["anim", "braille"] {
            if let Some(chars) = sets.get(name) {
                working_line_glyphs.extend(chars.iter().copied());
            }
        }

        let limits = &config.engine.limits;
        let mut profiles: Vec<Profile> = Vec::new();
        for raw in config.profiles {
            if profiles.len() >= limits.max_profiles {
                warn(format!(
                    "profile `{}` skipped: exceeds max_profiles ({})",
                    raw.id, limits.max_profiles
                ));
                continue;
            }
            match compile_profile(raw, &sets, limits.max_rules_per_profile, limits.max_tail_lines) {
                Ok(p) => {
                    // Replace-by-id (later definition — i.e. a user override — wins).
                    if let Some(slot) = profiles.iter_mut().find(|p2| p2.id == p.id) {
                        *slot = p;
                    } else {
                        profiles.push(p);
                    }
                }
                Err(e) => warn(e),
            }
        }

        let agent_index = profiles.iter().position(|p| p.kind == "agent");
        Registry { profiles, working_line_glyphs, agent_index }
    }
}

fn compile_profile(
    raw: RawProfile,
    sets: &HashMap<String, Vec<char>>,
    max_rules: usize,
    max_tail: usize,
) -> Result<Profile, String> {
    let mut rules = Vec::new();
    for rr in raw.rules {
        if rules.len() >= max_rules {
            warn(format!(
                "profile `{}`: dropping rules past max_rules_per_profile ({max_rules})",
                raw.id
            ));
            break;
        }
        let status = parse_status(&rr.status)
            .ok_or_else(|| format!("profile `{}`: invalid status `{}`", raw.id, rr.status))?;
        let when = compile_when(rr.when, sets)
            .map_err(|e| format!("profile `{}`: {e}", raw.id))?;
        rules.push(Rule { status, confidence: rr.confidence, evidence: rr.evidence, when });
    }
    let model = raw
        .model
        .into_iter()
        .filter(|m| m.from == "screen")
        .map(|m| ModelExtract {
            badges: m
                .badges
                .into_iter()
                .map(|(needle, family)| (needle.to_ascii_lowercase(), family))
                .collect(),
        })
        .collect();
    Ok(Profile {
        id: raw.id,
        kind: raw.kind,
        priority: raw.priority,
        tail_lines: raw.tail_lines.clamp(1, max_tail),
        unknown_confidence: raw.unknown_confidence,
        commands: raw.identity.commands,
        screen_tokens: raw
            .identity
            .screen_tokens
            .into_iter()
            .map(|s| s.to_ascii_lowercase())
            .collect(),
        rules,
        model,
    })
}

fn parse_config(text: &str, origin: &str) -> Option<RawConfig> {
    match toml::from_str::<RawConfig>(text) {
        Ok(c) => Some(c),
        Err(e) => {
            warn(format!("{origin}: {e}"));
            None
        }
    }
}

fn user_profiles_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MUXPILOT_PROFILES") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("muxpilot").join("profiles.toml"))
}

/// Build the registry: embedded defaults first, then the user override file
/// (replace-by-id) layered on top. A bad user file is logged and ignored; the
/// embedded defaults always classify.
fn load() -> Registry {
    let mut config = parse_config(DEFAULT_PROFILES, "default-profiles.toml")
        .expect("compiled-in default-profiles.toml must parse");

    if let Some(path) = user_profiles_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(user) = parse_config(&text, &path.display().to_string()) {
                // Later profiles override by id; merge spinner sets too.
                config.spinner_sets.extend(user.spinner_sets);
                config.profiles.extend(user.profiles);
                if user.engine.limits.max_profiles != d_max_profiles() {
                    config.engine.limits.max_profiles = user.engine.limits.max_profiles;
                }
            }
        }
    }

    Registry::compile(config)
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// The process-wide profile registry, loaded once on first use.
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(load)
}

/// Diagnostic sink for load-time problems. Best-effort to stderr — a malformed
/// profile degrades that profile, it never crashes the picker.
fn warn(msg: String) {
    eprintln!("muxpilot: profiles: {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> Registry {
        Registry::compile(
            parse_config(DEFAULT_PROFILES, "test").expect("default profiles parse"),
        )
    }

    fn out() -> OutputSignals {
        OutputSignals::default()
    }

    #[test]
    fn default_profiles_parse_and_expose_agent_and_fallback() {
        let r = reg();
        assert!(r.agent_index.is_some(), "an agent profile must exist");
        assert!(
            r.profiles.iter().any(|p| p.id == "generic-shell"),
            "a shell fallback profile must exist"
        );
        // The union used for working-line detection includes both glyph families.
        assert!(r.working_line_glyphs.contains(&'✽'));
        assert!(r.working_line_glyphs.contains(&'⠹'));
    }

    #[test]
    fn agent_classification_matches_the_old_tables() {
        use PaneAgentStatus::*;
        let r = reg();
        let s = |t: &str| r.classify_agent(t, out());
        assert_eq!(s("⠹ Thinking… (esc to interrupt)").status, Working);
        assert_eq!(s("out\n· Running a tool (esc to stop)").status, Working);
        // Approval outranks a spinner rendered alongside it.
        let appr = s("⠋ Do you want to proceed?\n❯ 1. Yes\n  2. No");
        assert_eq!(appr.status, WaitingApprove);
        assert_eq!(appr.confidence, 80);
        assert!(!appr.evidence.is_empty());
        assert_eq!(s("Type your answer (esc to cancel)").status, WaitingInput);
        assert_eq!(s("done.\n╭─────╮\n│ >   │\n╰─────╯").status, Idle);
        // Nothing matched → Unknown at the fallback confidence.
        let unk = s("ran: cat a > b.txt\nplain text");
        assert_eq!((unk.status, unk.confidence), (Unknown, 50));
    }

    #[test]
    fn agent_working_line_needs_the_ellipsis() {
        use PaneAgentStatus::*;
        let r = reg();
        let working = "✽ Sketching… (14m)\n────\n❯\n────\n  Op1M  780k left";
        assert_eq!(r.classify_agent(working, out()).status, Working);
        // Same glyph family, no ellipsis, empty prompt below → idle, not working.
        let done = "✻ Churned for 2m 33s\n────\n❯\n────\n  Op1M  610k left";
        assert_eq!(r.classify_agent(done, out()).status, Idle);
    }

    #[test]
    fn model_scrape_reads_badge_family() {
        let r = reg();
        assert_eq!(
            r.model_from_screen("🤖 Op1M 🔥  780k left  76%").as_deref(),
            Some("~opus")
        );
        assert_eq!(
            r.model_from_screen("… Sonnet  120k left").as_deref(),
            Some("~sonnet")
        );
        assert_eq!(r.model_from_screen("$ ls -la\n$ "), None);
    }

    #[test]
    fn beyond_agents_surface_process_states() {
        use PaneAgentStatus::*;
        let r = reg();
        // terraform waiting for approval.
        let (kind, c) = r
            .classify_process(
                "terraform",
                "Plan: 3 to add.\n\nDo you want to perform these actions?\n  Enter a value: ",
                out(),
            )
            .expect("terraform approval classifies");
        assert_eq!(kind, "deploy");
        assert_eq!(c.status, WaitingApprove);

        // cargo build error.
        let (_, c) = r
            .classify_process("cargo", "   Compiling foo\nerror[E0433]: boom", out())
            .expect("cargo error classifies");
        assert_eq!(c.status, Error);

        // pytest failures beat the passed line.
        let (_, c) = r
            .classify_process(
                "pytest",
                "collected 3 items\n=== FAILURES ===\n1 failed, 2 passed",
                out(),
            )
            .expect("pytest failure classifies");
        assert_eq!(c.status, Error);

        // ssh host-key prompt via the generic-shell fallback.
        let (kind, c) = r
            .classify_process(
                "zsh",
                "The authenticity of host 'x' can't be established.\nAre you sure you want to continue connecting (yes/no)? ",
                out(),
            )
            .expect("ssh prompt classifies");
        assert_eq!(kind, "shell");
        assert_eq!(c.status, WaitingApprove);

        // A plain idle shell identifies but fires no rule → None (no noise).
        assert!(r.classify_process("bash", "$ ls\nfile.txt\n$ ", out()).is_none());
        // An unknown command isn't classified at all.
        assert!(r.classify_process("htop", "lots of bars", out()).is_none());
    }

    #[test]
    fn user_override_replaces_by_id() {
        // A user profile with an existing id replaces it rather than duplicating.
        let user = r#"
[[profile]]
id = "agent"
kind = "agent"
tail_lines = 5
unknown_confidence = 7
"#;
        let mut config = parse_config(DEFAULT_PROFILES, "test").unwrap();
        let u = parse_config(user, "user").unwrap();
        config.profiles.extend(u.profiles);
        let r = Registry::compile(config);
        let agent_count = r.profiles.iter().filter(|p| p.kind == "agent").count();
        assert_eq!(agent_count, 1, "override replaces, not appends");
        // The override's fallback confidence now drives the no-match result.
        assert_eq!(r.classify_agent("nothing here", out()).confidence, 7);
    }

    #[test]
    fn invalid_status_is_rejected_not_fatal() {
        let bad = r#"
[[profile]]
id = "broken"
kind = "test"
[[profile.rule]]
status = "Compiling"
confidence = 60
when.screen_any_substring_i = ["x"]
"#;
        let config = parse_config(bad, "test").unwrap();
        // Compilation drops the broken profile but does not panic.
        let r = Registry::compile(config);
        assert!(r.profiles.iter().all(|p| p.id != "broken"));
    }
}
