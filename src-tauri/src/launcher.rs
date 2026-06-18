use crate::profiles::Loadout;

/// Disable Claude's bundled skills for the session (passed as a `--settings` value).
pub const DISABLE_BUNDLED: &str = r#"{"disableBundledSkills":true}"#;

/// The agent program to invoke. On Windows we name the extension explicitly so
/// PATHEXT can't resolve `codex` to the (execution-policy-blocked) `codex.ps1`
/// shim instead of `codex.cmd`. `claude` ships a native `claude.exe`.
pub fn resolve_exe(agent: &str) -> String {
    match (agent, cfg!(windows)) {
        ("codex", true) => "codex.cmd".into(),
        ("codex", false) => "codex".into(),
        (_, true) => "claude.exe".into(),
        (_, false) => "claude".into(),
    }
}

/// Is this a headless (one-shot) run? Claude with a prompt uses `-p`; Codex with
/// a prompt uses `exec`. Headless runs need the terminal held open afterward.
pub fn is_headless(p: &Loadout) -> bool {
    prompt_of(p).is_some()
}

fn prompt_of(p: &Loadout) -> Option<&str> {
    match p.prompt.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

/// Build the Claude argv (program excluded — see `resolve_exe`). `skills_root` is
/// the dir Skillet populated with the selected skills under `.claude/skills/`;
/// `mcp_config` is the path to the generated strict MCP config.
pub fn claude_argv(p: &Loadout, skills_root: &str, mcp_config: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--setting-sources".into(),
        "project".into(),
        "--add-dir".into(),
        skills_root.into(),
        "--settings".into(),
        DISABLE_BUNDLED.into(),
        "--mcp-config".into(),
        mcp_config.into(),
        "--strict-mcp-config".into(),
    ];
    if let Some(m) = p.model.as_deref().filter(|s| !s.is_empty()) {
        a.push("--model".into());
        a.push(m.into());
    }
    if let Some(e) = p.effort.as_deref().filter(|s| !s.is_empty()) {
        a.push("--effort".into());
        a.push(e.into());
    }
    if let Some(prompt) = prompt_of(p) {
        a.push("-p".into());
        a.push(prompt.into());
    }
    if p.skip_perms {
        a.push("--dangerously-skip-permissions".into());
    }
    a
}

/// Build the Codex argv (program excluded). Skills/MCP do not map to Codex.
/// Headless (`prompt` set) uses the `exec` subcommand; otherwise interactive.
pub fn codex_argv(p: &Loadout, workdir: &str) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    if prompt_of(p).is_some() {
        a.push("exec".into());
    }
    a.push("-C".into());
    a.push(workdir.into());
    if let Some(m) = p.model.as_deref().filter(|s| !s.is_empty()) {
        a.push("-m".into());
        a.push(m.into());
    }
    if p.skip_perms {
        a.push("--dangerously-bypass-approvals-and-sandbox".into());
    }
    if let Some(prompt) = prompt_of(p) {
        a.push(prompt.into());
    }
    a
}

// ---------------- launcher-script encoders ----------------
//
// Security: user input (prompt, paths, skill names) NEVER becomes shell syntax.
// Each token is emitted inside a single-quoted string literal where the only
// special character is the single quote itself, which we escape. So `;`, `"`,
// `$()`, backticks, newlines all pass through verbatim — no injection.

/// PowerShell single-quote: wrap in `'…'`, double any internal `'`.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// POSIX single-quote: wrap in `'…'`, replace internal `'` with `'\''`.
pub fn posix_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Render a Windows `.ps1` launcher. Started via
/// `powershell -NoProfile -ExecutionPolicy Bypass -NoExit -File <this>`.
pub fn render_ps1(exe: &str, args: &[String], workdir: &str, keep_open: bool) -> String {
    let mut s = String::new();
    s.push_str("$ErrorActionPreference = 'Stop'\n");
    s.push_str(&format!("Set-Location -LiteralPath {}\n", ps_quote(workdir)));
    s.push_str("$cmdArgs = @(\n");
    for a in args {
        s.push_str(&format!("  {},\n", ps_quote(a)));
    }
    s.push_str(")\n");
    s.push_str(&format!("& {} @cmdArgs\n", ps_quote(exe)));
    if keep_open {
        s.push_str("Read-Host 'Press Enter to close'\n");
    }
    s
}

/// Render a POSIX launcher (`.command` on macOS, `.sh` on Linux).
pub fn render_posix(exe: &str, args: &[String], workdir: &str, keep_open: bool) -> String {
    let mut s = String::from("#!/bin/sh\n");
    s.push_str(&format!("cd {} || exit 1\n", posix_quote(workdir)));
    s.push_str(&posix_quote(exe));
    for a in args {
        s.push(' ');
        s.push_str(&posix_quote(a));
    }
    s.push('\n');
    if keep_open {
        s.push_str("printf 'Press Enter to close'\nread _\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{McpRef, SkillRef};

    fn base() -> Loadout {
        Loadout {
            id: "x".into(),
            name: "x".into(),
            note: String::new(),
            agent: "claude".into(),
            workdir: "C:/proj".into(),
            skills: vec![SkillRef {
                name: "verify".into(),
                scope: "personal".into(),
                plugin: None,
                path: String::new(),
            }],
            mcps: vec![McpRef {
                name: "github".into(),
                scope: "user".into(),
                source_path: String::new(),
            }],
            model: None,
            effort: None,
            prompt: None,
            skip_perms: false,
            builtin: false,
        }
    }

    #[test]
    fn claude_argv_core_flags_present_and_ordered() {
        let a = claude_argv(&base(), "ROOT", "MCP");
        assert_eq!(
            a,
            vec![
                "--setting-sources",
                "project",
                "--add-dir",
                "ROOT",
                "--settings",
                DISABLE_BUNDLED,
                "--mcp-config",
                "MCP",
                "--strict-mcp-config",
            ]
        );
    }

    #[test]
    fn claude_argv_optionals() {
        let mut p = base();
        p.model = Some("opus".into());
        p.effort = Some("high".into());
        p.prompt = Some("do it".into());
        p.skip_perms = true;
        let a = claude_argv(&p, "ROOT", "MCP");
        assert!(a.windows(2).any(|w| w == ["--model", "opus"]));
        assert!(a.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(a.windows(2).any(|w| w == ["-p", "do it"]));
        assert!(a.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn claude_argv_blank_prompt_is_interactive() {
        let mut p = base();
        p.prompt = Some("   ".into());
        assert!(!claude_argv(&p, "R", "M").contains(&"-p".to_string()));
        assert!(!is_headless(&p));
    }

    #[test]
    fn codex_interactive_has_no_exec() {
        let mut p = base();
        p.agent = "codex".into();
        p.model = Some("gpt-5.5".into());
        let a = codex_argv(&p, "C:/proj");
        assert_eq!(a, vec!["-C", "C:/proj", "-m", "gpt-5.5"]);
    }

    #[test]
    fn codex_headless_uses_exec_and_trailing_prompt() {
        let mut p = base();
        p.agent = "codex".into();
        p.prompt = Some("review".into());
        p.skip_perms = true;
        let a = codex_argv(&p, "C:/proj");
        assert_eq!(a[0], "exec");
        assert_eq!(&a[1..3], ["-C", "C:/proj"]);
        assert!(a.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert_eq!(a.last().unwrap(), "review");
    }

    #[test]
    fn resolve_exe_platform() {
        if cfg!(windows) {
            assert_eq!(resolve_exe("codex"), "codex.cmd");
            assert_eq!(resolve_exe("claude"), "claude.exe");
        } else {
            assert_eq!(resolve_exe("codex"), "codex");
            assert_eq!(resolve_exe("claude"), "claude");
        }
    }

    // --- injection-safety: the heart of §4.4 ---

    #[test]
    fn ps_quote_escapes_single_quotes_only() {
        assert_eq!(ps_quote("a'b"), "'a''b'");
        // metachars stay literal inside single quotes
        assert_eq!(ps_quote("; rm -rf /"), "'; rm -rf /'");
        assert_eq!(ps_quote("$(whoami)`x\""), "'$(whoami)`x\"'");
    }

    #[test]
    fn posix_quote_escapes_single_quotes() {
        assert_eq!(posix_quote("a'b"), "'a'\\''b'");
        assert_eq!(posix_quote("; rm -rf /"), "'; rm -rf /'");
    }

    #[test]
    fn ps1_malicious_prompt_does_not_break_out() {
        let mut p = base();
        let evil = "'; Remove-Item C:\\ -Recurse; echo '";
        p.prompt = Some(evil.into());
        let args = claude_argv(&p, "ROOT", "MCP");
        let script = render_ps1("claude.exe", &args, "C:/a b/dir", true);
        // The evil prompt is fully contained in one quoted literal (doubled quotes),
        // never as bare PowerShell tokens.
        assert!(script.contains("'''; Remove-Item C:\\ -Recurse; echo '''"));
        // No unescaped breakout: the raw Remove-Item must not appear outside quotes.
        assert!(!script.contains("\n'; Remove-Item"));
        assert!(script.contains("Read-Host"));
    }

    #[test]
    fn posix_script_handles_newline_and_semicolons() {
        let mut p = base();
        p.prompt = Some("line1\n; rm -rf ~".into());
        let args = claude_argv(&p, "ROOT", "MCP");
        let script = render_posix("claude", &args, "/home/u/p", false);
        // newline + semicolons live inside a single-quoted token, harmless
        assert!(script.contains("'line1\n; rm -rf ~'"));
        assert!(!script.contains("read _")); // not keep_open
    }
}
