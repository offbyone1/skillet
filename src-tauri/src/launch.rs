use crate::launcher;
use crate::mcp;
use crate::profiles::{Loadout, SkillRef};
use crate::skills;
use crate::util;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

// Skill-copy safety caps (a skill dir could be huge or symlink-laced).
const MAX_DEPTH: u32 = 24;
const MAX_FILES: u32 = 5000;
const MAX_BYTES: u64 = 50 * 1024 * 1024;

// Launch-dirs older than this are swept at app start (never while a session may
// still read them — only on startup).
const CLEANUP_TTL_SECS: u64 = 24 * 60 * 60;

/// Resolve the effective working directory for a launch.
fn resolve_workdir(p: &Loadout, override_dir: Option<String>) -> Result<PathBuf, String> {
    let raw = override_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| p.workdir.clone());
    if raw.trim().is_empty() {
        return Err("Kein Arbeitsverzeichnis gesetzt.".into());
    }
    let dir = PathBuf::from(&raw);
    if !dir.is_dir() {
        return Err(format!("Arbeitsverzeichnis existiert nicht: {raw}"));
    }
    Ok(dir)
}

/// Trust-relevant elements present in `workdir` — Skillet loads the project
/// setting source, so these execute/influence the agent. The UI surfaces this
/// before launching with `-p` or `skip_perms` (§4.2).
pub fn preflight_scan(workdir: &str) -> Vec<String> {
    let dir = Path::new(workdir);
    let mut found = Vec::new();
    let probes: &[(&str, &str)] = &[
        (".claude/settings.json", "Projekt-Settings"),
        (".claude/settings.local.json", "Lokale Settings"),
        (".claude/hooks", "Hooks"),
        ("CLAUDE.md", "CLAUDE.md"),
        ("CLAUDE.local.md", "CLAUDE.local.md"),
        (".mcp.json", "Projekt-MCP"),
        (".claude/skills", "Projekt-Skills"),
    ];
    for (rel, label) in probes {
        if dir.join(rel).exists() {
            found.push(label.to_string());
        }
    }
    found
}

/// Launch an agent for `profile`. Returns a short human status on success.
pub fn run(profile: &Loadout, workdir_override: Option<String>) -> Result<String, String> {
    let workdir = resolve_workdir(profile, workdir_override)?;
    let workdir_str = workdir.to_string_lossy().to_string();
    let run_dir = make_run_dir()?;
    let runtime = run_dir.join("runtime");
    fs::create_dir_all(&runtime).map_err(|e| format!("runtime dir: {e}"))?;
    restrict_perms(&runtime);
    write_lock(&runtime);

    let exe = launcher::resolve_exe(&profile.agent);
    let keep_open = launcher::is_headless(profile);

    let args = if profile.agent == "codex" {
        launcher::codex_argv(profile, &workdir_str)
    } else {
        let skills_root = run_dir.join("skills-root");
        project_skills_into(&skills_root, &profile.skills)?;
        let mcp_path = runtime.join("mcp.json");
        write_mcp_config(&mcp_path, &workdir, &profile)?;
        restrict_perms(&mcp_path);
        launcher::claude_argv(
            profile,
            &skills_root.to_string_lossy(),
            &mcp_path.to_string_lossy(),
        )
    };

    let script = write_launcher(&runtime, &exe, &args, &workdir_str, keep_open)?;
    spawn_terminal(&script, &workdir)?;
    Ok(format!("{} gestartet in {}", profile.agent, workdir_str))
}

/// `<~/.skillet>/launch/<run-id>` — unique per launch (time + pid).
fn make_run_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let id = format!("{}-{}", nanos, std::process::id());
    let dir = crate::profiles::skillet_dir().join("launch").join(id);
    fs::create_dir_all(&dir).map_err(|e| format!("launch dir: {e}"))?;
    Ok(dir)
}

/// Copy the selected skills into `<skills_root>/.claude/skills/<name>/`. Resolves
/// each ref to a real source dir (its `path`, else by name via the live scan).
fn project_skills_into(skills_root: &Path, refs: &[SkillRef]) -> Result<(), String> {
    let dest_base = skills_root.join(".claude").join("skills");
    fs::create_dir_all(&dest_base).map_err(|e| format!("skills dir: {e}"))?;
    if refs.is_empty() {
        return Ok(());
    }
    let scanned = skills::scan_all();
    for r in refs {
        let src = resolve_skill_src(r, &scanned)
            .ok_or_else(|| format!("Skill nicht gefunden: {}", r.name))?;
        let dest = dest_base.join(&r.name);
        copy_tree(&src, &dest, 0, &mut 0, &mut 0)?;
    }
    Ok(())
}

fn resolve_skill_src(r: &SkillRef, scanned: &[skills::Skill]) -> Option<PathBuf> {
    if !r.path.is_empty() {
        let p = PathBuf::from(&r.path);
        if p.is_dir() {
            return Some(p);
        }
    }
    // builtin templates carry no path: match by name (+ scope when it helps).
    scanned
        .iter()
        .find(|s| s.name == r.name && s.scope == r.scope)
        .or_else(|| scanned.iter().find(|s| s.name == r.name))
        .map(|s| PathBuf::from(&s.path))
}

/// Hardened recursive copy: rejects symlinks/junctions, caps depth/files/bytes.
fn copy_tree(
    src: &Path,
    dest: &Path,
    depth: u32,
    files: &mut u32,
    bytes: &mut u64,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("Skill zu tief verschachtelt: {}", src.display()));
    }
    let meta = fs::symlink_metadata(src).map_err(|e| format!("stat {}: {e}", src.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("Symlink in Skill abgelehnt: {}", src.display()));
    }
    fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
        if m.file_type().is_symlink() {
            return Err(format!("Symlink in Skill abgelehnt: {}", p.display()));
        }
        let target = dest.join(entry.file_name());
        if m.is_dir() {
            copy_tree(&p, &target, depth + 1, files, bytes)?;
        } else {
            *files += 1;
            *bytes += m.len();
            if *files > MAX_FILES || *bytes > MAX_BYTES {
                return Err(format!("Skill überschreitet Limits: {}", src.display()));
            }
            fs::copy(&p, &target).map_err(|e| format!("copy {}: {e}", p.display()))?;
        }
    }
    Ok(())
}

/// Write the strict `--mcp-config` file. Errors if a selected server can't be
/// reconstructed (plugin/managed) — the UI should never offer those, so this is
/// the defensive backstop.
fn write_mcp_config(path: &Path, workdir: &Path, profile: &Loadout) -> Result<(), String> {
    let (servers, unresolved) = mcp::resolve_specs(workdir, &profile.mcps);
    if !unresolved.is_empty() {
        return Err(format!(
            "MCP-Server nicht isolierbar: {}",
            unresolved.join(", ")
        ));
    }
    let json = serde_json::json!({ "mcpServers": servers });
    let pretty = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(path, pretty).map_err(|e| format!("mcp.json: {e}"))
}

/// Write the platform launcher script into `runtime/`, returning its path.
fn write_launcher(
    runtime: &Path,
    exe: &str,
    args: &[String],
    workdir: &str,
    keep_open: bool,
) -> Result<PathBuf, String> {
    let (name, body) = if cfg!(windows) {
        ("launcher.ps1", launcher::render_ps1(exe, args, workdir, keep_open))
    } else if cfg!(target_os = "macos") {
        ("launcher.command", launcher::render_posix(exe, args, workdir, keep_open))
    } else {
        ("launcher.sh", launcher::render_posix(exe, args, workdir, keep_open))
    };
    let path = runtime.join(name);
    fs::write(&path, body).map_err(|e| format!("launcher: {e}"))?;
    make_executable(&path);
    Ok(path)
}

fn spawn_terminal(script: &Path, workdir: &Path) -> Result<(), String> {
    let s = script.to_string_lossy().to_string();
    let wd = workdir.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        let wt = Command::new("wt.exe")
            .args([
                "-d", &wd, "--", "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass",
                "-NoExit", "-File", &s,
            ])
            .spawn();
        if wt.is_ok() {
            return Ok(());
        }
        // Fallback: PowerShell in its own new console window.
        Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-NoExit", "-File", &s])
            .current_dir(workdir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| format!("Terminal-Start fehlgeschlagen: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let _ = wd;
        Command::new("open")
            .args(["-a", "Terminal", &s])
            .spawn()
            .map_err(|e| format!("Terminal-Start fehlgeschlagen: {e}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let term = std::env::var("TERMINAL").unwrap_or_default();
        let candidates: Vec<&str> = if term.is_empty() {
            vec!["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
        } else {
            vec![term.as_str()]
        };
        for t in candidates {
            if Command::new(t)
                .args(["-e", "sh", &s])
                .current_dir(workdir)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
        Err("Kein Terminal-Emulator gefunden.".into())
    }
}

/// Sweep stale launch dirs at app start — only ones older than the TTL, so an
/// open session's dir is never pulled out from under it.
pub fn cleanup_old() {
    let base = crate::profiles::skillet_dir().join("launch");
    let now = util::now_secs();
    let entries = match fs::read_dir(&base) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let age = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| now.saturating_sub(d.as_secs()))
            .unwrap_or(0);
        if age > CLEANUP_TTL_SECS {
            let _ = fs::remove_dir_all(&p);
        }
    }
}

fn write_lock(runtime: &Path) {
    let _ = fs::write(runtime.join(".lock"), util::now_secs().to_string());
}

#[cfg(unix)]
fn restrict_perms(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mode = if meta.is_dir() { 0o700 } else { 0o600 };
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
}

/// Windows: `~/.skillet` lives under the user profile, whose default ACL already
/// restricts access to the owner. Tightening per-file ACLs needs winapi; v1
/// relies on the profile-dir ACL. (See spec §8.)
#[cfg(not(unix))]
fn restrict_perms(_path: &Path) {}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
