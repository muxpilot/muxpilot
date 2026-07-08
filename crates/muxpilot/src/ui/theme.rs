//! Re-export the palette types so `ui::*` consumers can take a `&Theme` and read
//! its fields directly. The picker threads one `&Theme` through every draw call,
//! which is what makes the runtime light/dark toggle possible.
pub(crate) use crate::native_state::{default_theme, Theme};
