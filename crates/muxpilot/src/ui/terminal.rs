use crate::error::{AppError, ErrorCode};
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

pub(crate) fn terminal_error(
    context: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> AppError {
    AppError::new(ErrorCode::ProviderFailure, context)
        .runtime("crossterm")
        .with_source(error)
}

pub(crate) struct CrosstermGuard;

impl CrosstermGuard {
    pub(crate) fn enter() -> Result<Self, AppError> {
        enable_raw_mode().map_err(|e| terminal_error("failed to enable raw mode", e))?;
        execute!(std::io::stdout(), EnterAlternateScreen, Hide)
            .map_err(|e| terminal_error("failed to enter alternate screen", e))?;
        Ok(Self)
    }
}

impl Drop for CrosstermGuard {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
