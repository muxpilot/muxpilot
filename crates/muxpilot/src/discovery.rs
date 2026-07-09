use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::model::*;

pub(crate) fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

pub(crate) fn in_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

/// Run a command and capture stdout lines, treating any failure as empty output
/// (mirrors the script's `runCommand` swallowing errors).
async fn capture_lines(args: &[&str]) -> Vec<String> {
    capture_text(args)
        .await
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect()
}

/// Run a command and capture trimmed stdout, treating failure as empty.
async fn capture_text(args: &[&str]) -> String {
    let Some((command, rest)) = args.split_first() else {
        return String::new();
    };
    std::process::Command::new(command)
        .args(rest)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

pub(crate) async fn tmux_sessions() -> Vec<String> {
    capture_lines(&["tmux", "list-sessions", "-F", "#{session_name}"]).await
}

async fn tmux_sessions_with_windows() -> Vec<SessionItem> {
    capture_lines(&[
        "tmux",
        "list-sessions",
        "-F",
        "#{session_name}\t#{session_windows}",
    ])
    .await
    .into_iter()
    .map(|line| {
        let (name, windows) = line.split_once('\t').unwrap_or((line.as_str(), "0"));
        SessionItem {
            name: name.to_string(),
            windows: windows.parse::<u32>().unwrap_or(0),
        }
    })
    .collect()
}

async fn current_session() -> String {
    capture_text(&["tmux", "display-message", "-p", "#{session_name}"]).await
}

async fn tmuxinator_projects() -> Vec<String> {
    let config_home =
        std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home()));
    let dir = format!("{config_home}/tmuxinator");
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("yml"))
                .then(|| path.file_name()?.to_str().map(str::to_string))
                .flatten()
        })
        .filter_map(|name| tmuxinator_project_name(&name))
        .collect()
}

async fn zoxide_dirs(home: &str) -> Vec<DirItem> {
    capture_lines(&["zoxide", "query", "--list"])
        .await
        .into_iter()
        .take(20)
        .map(|path| DirItem {
            display: tilde(&path, home),
            path,
            has_local_config: false,
        })
        .collect()
}

async fn git_repos(home: &str) -> Vec<DirItem> {
    let github =
        std::env::var("GIT_REPOS_GITHUB").unwrap_or_else(|_| format!("{home}/gits/github"));
    let gitlab =
        std::env::var("GIT_REPOS_GITLAB").unwrap_or_else(|_| format!("{home}/gits/gitlab"));

    let mut repos = Vec::new();
    for base in [github, gitlab] {
        if !Path::new(&base).is_dir() {
            continue;
        }
        // org/repo structure: exactly two levels deep. `find` bounds the depth
        // cheaply; a recursive glob would walk entire repo trees.
        let paths = capture_lines(&[
            "find",
            &base,
            "-mindepth",
            "2",
            "-maxdepth",
            "2",
            "-type",
            "d",
        ])
        .await;
        for path in paths {
            repos.push(DirItem {
                display: tilde(&path, home),
                has_local_config: false,
                path,
            });
        }
    }
    repos
}

pub(crate) fn agentvibes_layout(path: &str) -> String {
    format!("{path}/.agentvibes/tmux.yml")
}

pub(crate) fn classic_layout(path: &str) -> String {
    format!("{path}/.tmuxinator.yml")
}

/// The on-disk yaml a repo-local layout is started from: the `.agentvibes`
/// convergence file when present, else the classic `.tmuxinator.yml`. Matches
/// the launcher's preference so the picker shows the file it would actually run.
pub(crate) fn resolve_local_layout_file(dir: &str) -> String {
    let agentvibes = agentvibes_layout(dir);
    if std::path::Path::new(&agentvibes).exists() {
        agentvibes
    } else {
        classic_layout(dir)
    }
}

/// The global tmuxinator config file for a saved project name
/// (`$XDG_CONFIG_HOME/tmuxinator/<name>.yml`, matching discovery).
pub(crate) fn tmuxinator_project_file(name: &str) -> String {
    let config_home =
        std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home()));
    format!("{config_home}/tmuxinator/{name}.yml")
}

/// A directory is launchable if it has a classic `.tmuxinator.yml` or our
/// convergence `.agentvibes/tmux.yml`.
fn check_local_config(path: &str) -> bool {
    Path::new(&agentvibes_layout(path)).is_file() || Path::new(&classic_layout(path)).is_file()
}

/// The tmux session name a repo-local layout would create, falling back to the
/// sanitized directory name.
fn read_layout_name(path: &str) -> String {
    for rel in [agentvibes_layout(path), classic_layout(path)] {
        if let Ok(text) = std::fs::read_to_string(&rel) {
            if let Some(name) = parse_layout_name(&text) {
                return name;
            }
        }
    }
    sanitize_session_name(path)
}

/// Gather everything and assemble the render model.
async fn build_model_uncached() -> MenuModel {
    let home = home();
    let sessions = tmux_sessions_with_windows().await;
    let projects = tmuxinator_projects().await;
    let mut zoxide = zoxide_dirs(&home).await;
    let mut repos = git_repos(&home).await;
    let current = current_session().await;

    for d in zoxide.iter_mut() {
        d.has_local_config = check_local_config(&d.path);
    }
    for r in repos.iter_mut() {
        r.has_local_config = check_local_config(&r.path);
    }

    let running: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.name.as_str()).collect();

    let layouts: Vec<Layout> = repos
        .iter()
        .filter(|r| r.has_local_config)
        .map(|r| {
            let session = read_layout_name(&r.path);
            let running = running.contains(session.as_str());
            Layout {
                session,
                display: r.display.clone(),
                path: r.path.clone(),
                running,
            }
        })
        .collect();

    let plain_repos: Vec<DirItem> = repos.into_iter().filter(|r| !r.has_local_config).collect();

    MenuModel {
        sessions,
        current,
        layouts,
        projects,
        zoxide,
        plain_repos,
    }
}

fn model_cache_path() -> String {
    std::env::var("TMUXINATOR_MENU_CACHE").unwrap_or_else(|_| {
        format!(
            "{}/muxpilot-model.json",
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
        )
    })
}

fn cache_ttl() -> Duration {
    std::env::var("TMUXINATOR_MENU_CACHE_TTL_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(2500))
}

fn read_cached_model() -> Option<MenuModel> {
    let path = model_cache_path();
    let meta = std::fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()? > cache_ttl() {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_cached_model(model: &MenuModel) {
    let path = model_cache_path();
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(model) {
        let _ = std::fs::write(path, text);
    }
}

pub(crate) async fn build_model_cached(no_cache: bool) -> MenuModel {
    if !no_cache {
        if let Some(model) = read_cached_model() {
            return model;
        }
    }
    let model = build_model_uncached().await;
    write_cached_model(&model);
    model
}
