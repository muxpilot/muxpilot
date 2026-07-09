//! Tiny cross-run picker state: which tab (mode) the user was last on.
//!
//! Persisted to `$XDG_STATE_HOME/muxpilot/last-mode` (falling back to
//! `~/.local/state/muxpilot/last-mode`) so re-opening the picker lands on the
//! same tab — e.g. if you picked an agent last time, the Agents tab is active
//! again next time. Best-effort: any IO error silently degrades to the default.

use std::path::PathBuf;

use crate::native_state::PickerMode;

fn state_dir() -> PathBuf {
    let base = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/.local/state", crate::discovery::home()));
    PathBuf::from(base).join("muxpilot")
}

fn last_mode_path() -> PathBuf {
    state_dir().join("last-mode")
}

/// The tab to open on. Returns `None` when nothing was ever saved (or it is
/// unreadable), so the caller can pick its own default.
pub(crate) fn load_last_mode() -> Option<PickerMode> {
    std::fs::read_to_string(last_mode_path())
        .ok()
        .and_then(|s| PickerMode::from_key(&s))
}

/// Remember `mode` as the tab to reopen on. Best-effort; failures are ignored.
pub(crate) fn save_last_mode(mode: PickerMode) {
    let dir = state_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("last-mode"), mode.as_key());
}
