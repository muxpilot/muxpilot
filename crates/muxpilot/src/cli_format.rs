use crate::error::{AppError, ErrorCode};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Human,
    Json,
}

pub fn args_want_json(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json" || arg == "json")
        || args
            .windows(2)
            .any(|pair| matches!(pair, [flag, value] if (flag == "--format" || flag == "-o") && value == "json"))
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--format=json" | "-o=json"))
}

pub(crate) fn output_format(args: &[String]) -> Result<OutputFormat, AppError> {
    let mut format = if args_want_json(args) {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    for pair in args.windows(2) {
        if pair[0] == "--format" || pair[0] == "-o" {
            format = parse_output_format(&pair[1])?;
        }
    }
    for arg in args {
        if let Some(value) = arg
            .strip_prefix("--format=")
            .or_else(|| arg.strip_prefix("-o="))
        {
            format = parse_output_format(value)?;
        }
    }
    Ok(format)
}

fn parse_output_format(value: &str) -> Result<OutputFormat, AppError> {
    match value {
        "human" | "text" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        other => Err(AppError::new(
            ErrorCode::Validation,
            format!("unsupported output format: {other}"),
        )
        .op("muxpilot.output-format")),
    }
}

pub(crate) fn print_json(value: impl Serialize, operation: &str) -> Result<(), AppError> {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| {
            AppError::new(
                ErrorCode::ProviderFailure,
                format!("failed to encode JSON output: {e}"),
            )
            .op(operation)
        })?
    );
    Ok(())
}
