use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_errors = muxpilot::args_want_json(&args);
    match muxpilot::run(args).await {
        Ok(code) => code,
        Err(e) => {
            if json_errors {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "error": {
                            "message": e.to_string(),
                        }
                    })
                );
            } else {
                eprintln!("muxpilot: {e}");
            }
            ExitCode::FAILURE
        }
    }
}
