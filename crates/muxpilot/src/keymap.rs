//! Reconfigurable command-mode key bindings.
//!
//! The picker's event loop no longer hardcodes `KeyCode::Char('j')`-style checks
//! for its navigation/action keys. Instead every command key resolves through a
//! [`Keymap`] table to a semantic [`Action`], so a binding can be remapped by
//! swapping the table — the loop only knows about `Action`s.
//!
//! Scope: this covers **command mode** (browsing the list). Filter-line editing
//! (readline keys) and help-overlay scrolling are modal text/scroll contexts and
//! stay handled inline in the loop; they are not list bindings.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::native_state::PickerMode;

/// A semantic command the picker performs, decoupled from the physical key that
/// triggers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Activate the selected row (switch/start/attach).
    Open,
    /// Close the picker.
    Quit,
    /// Move the cursor down / up one row.
    Down,
    Up,
    /// Jump to the first / last row.
    Top,
    Bottom,
    /// Move a half page down / up.
    PageDown,
    PageUp,
    /// `t`/→ — expand the tree one level (or close an open one).
    ExpandLevel,
    /// Space — toggle the current tree node.
    ToggleLevel,
    /// ← — collapse the current tree level.
    CollapseLevel,
    /// Jump straight to a specific mode (`s`/`a`/`l`/`d`).
    SwitchMode(PickerMode),
    /// Tab — cycle to the next mode.
    NextMode,
    /// Enter filter-edit mode.
    EditFilter,
    /// Toggle the help overlay.
    ToggleHelp,
    /// Toggle the light/dark theme.
    ToggleTheme,
    /// Re-read tmux state.
    Refresh,
}

/// A `(key, modifiers) -> Action` binding table for command mode.
pub(crate) struct Keymap {
    bindings: Vec<(KeyCode, KeyModifiers, Action)>,
}

impl Keymap {
    /// The built-in default bindings. A future config loader can build a `Keymap`
    /// from user overrides instead; the loop is already agnostic to which one it
    /// holds.
    pub(crate) fn defaults() -> Self {
        use Action::*;
        use PickerMode::*;
        let n = KeyModifiers::NONE;
        let c = KeyModifiers::CONTROL;
        Self {
            bindings: vec![
                (KeyCode::Enter, n, Open),
                (KeyCode::Char('q'), n, Quit),
                (KeyCode::Char('c'), c, Quit),
                (KeyCode::Char('j'), n, Down),
                (KeyCode::Down, n, Down),
                (KeyCode::Char('k'), n, Up),
                (KeyCode::Up, n, Up),
                (KeyCode::Char('g'), n, Top),
                (KeyCode::Char('G'), n, Bottom),
                (KeyCode::Char('d'), c, PageDown),
                (KeyCode::Char('u'), c, PageUp),
                // Tree: `t` expands/collapses the node under the cursor (mnemonic
                // "tree"); the arrows still do granular expand/collapse and Space
                // toggles. `h`/`l` are no longer tree keys — `l` is now Layouts.
                (KeyCode::Char('t'), n, ExpandLevel),
                (KeyCode::Right, n, ExpandLevel),
                (KeyCode::Char(' '), n, ToggleLevel),
                (KeyCode::Left, n, CollapseLevel),
                // Mode switches are mnemonic: sessions/agents/layouts/dirs.
                (KeyCode::Char('s'), n, SwitchMode(Sessions)),
                (KeyCode::Char('a'), n, SwitchMode(Agents)),
                (KeyCode::Char('l'), n, SwitchMode(Layouts)),
                (KeyCode::Char('d'), n, SwitchMode(Dirs)),
                (KeyCode::Tab, n, NextMode),
                (KeyCode::Char('/'), n, EditFilter),
                (KeyCode::Char('?'), n, ToggleHelp),
                // Theme toggle moved to `T` (Shift+t) so `t` can mean "tree";
                // `resolve` strips SHIFT, so `Char('T')` matches without a modifier.
                (KeyCode::Char('T'), n, ToggleTheme),
                (KeyCode::Char('r'), n, Refresh),
            ],
        }
    }

    /// Resolve a key press to its command-mode action, if any. SHIFT is ignored
    /// because a `Char` already carries its case (so `G` matches without the
    /// caller having to special-case the shift modifier).
    pub(crate) fn resolve(&self, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        let mods = mods.difference(KeyModifiers::SHIFT);
        self.bindings
            .iter()
            .find(|(c, m, _)| *c == code && *m == mods)
            .map(|(_, _, action)| *action)
    }
}
