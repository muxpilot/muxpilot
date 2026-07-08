use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorCode {
    MissingIntegration,
    ProcessNonzeroExit,
    ProviderFailure,
    Validation,
}

#[derive(Debug)]
pub struct AppError {
    code: ErrorCode,
    message: String,
    operation: Option<String>,
    runtime: Option<String>,
    source: Option<String>,
}

impl AppError {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            operation: None,
            runtime: None,
            source: None,
        }
    }

    pub(crate) fn op(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    pub(crate) fn runtime(mut self, runtime: impl Into<String>) -> Self {
        self.runtime = Some(runtime.into());
        self
    }

    pub(crate) fn with_source(mut self, source: impl std::error::Error) -> Self {
        self.source = Some(source.to_string());
        self
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)?;
        if let Some(operation) = &self.operation {
            write!(f, " [op={operation}]")?;
        }
        if let Some(runtime) = &self.runtime {
            write!(f, " [runtime={runtime}]")?;
        }
        if let Some(source) = &self.source {
            write!(f, ": {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}
