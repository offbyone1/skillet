use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// User home directory. Windows: %USERPROFILE%, else $HOME.
pub fn home_dir() -> PathBuf {
    if let Ok(p) = std::env::var("USERPROFILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(p) = std::env::var("HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from(".")
}

/// Nearest ancestor of the current dir that looks like a project root —
/// contains `.mcp.json`, `.git`, or a `.claude/` dir. Returns None if none is
/// found before reaching the home dir.
///
/// A packaged Windows app can't be trusted to start in the project cwd, so we
/// search rather than assume. We stop at (and exclude) the home dir because the
/// global `~/.claude` would otherwise make home look like a project.
pub fn project_root() -> Option<PathBuf> {
    let home = home_dir();
    let start = std::env::current_dir().ok()?;
    let mut dir = start.as_path();
    loop {
        if dir == home {
            return None;
        }
        if dir.join(".mcp.json").is_file() || dir.join(".git").exists() || dir.join(".claude").is_dir()
        {
            return Some(dir.to_path_buf());
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => return None,
        }
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Compact relative age, e.g. "2h", "3d", "1w", "2mo". `ts` is epoch seconds.
pub fn rel_time(ts: u64) -> String {
    if ts == 0 {
        return "—".into();
    }
    let now = now_secs();
    let age = now.saturating_sub(ts);
    match age {
        a if a < 60 => "just now".into(),
        a if a < 3600 => format!("{}m", a / 60),
        a if a < 86_400 => format!("{}h", a / 3600),
        a if a < 7 * 86_400 => format!("{}d", a / 86_400),
        a if a < 30 * 86_400 => format!("{}w", a / (7 * 86_400)),
        a if a < 365 * 86_400 => format!("{}mo", a / (30 * 86_400)),
        a => format!("{}y", a / (365 * 86_400)),
    }
}

pub fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

const CLAUDE_TIMEOUT_SECS: u64 = 15;

/// Run the `claude` CLI cross-platform, capped by a timeout.
///
/// `claude mcp list` performs network health-checks and can hang; without a
/// timeout that would freeze the calling Tauri command. We run it on a worker
/// thread and give up after `CLAUDE_TIMEOUT_SECS` (the worker is abandoned —
/// the orphaned `claude` finishes on its own).
pub fn run_claude(args: &[&str]) -> Option<String> {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(exec_claude(&owned));
    });
    match rx.recv_timeout(Duration::from_secs(CLAUDE_TIMEOUT_SECS)) {
        Ok(Some(s)) => Some(s),
        _ => None, // timed out, errored, or worker died
    }
}

/// On Windows `claude` is a `.cmd` shim, so it must be invoked through `cmd /C`
/// (a bare `Command::new("claude")` fails). CREATE_NO_WINDOW suppresses the
/// console flash from a GUI process.
fn exec_claude(args: &[String]) -> Option<String> {
    let output = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("claude").args(args);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        cmd.output().ok()?
    } else {
        Command::new("claude").args(args).output().ok()?
    };
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn claude_version() -> String {
    match run_claude(&["--version"]) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "not found".into(),
    }
}
