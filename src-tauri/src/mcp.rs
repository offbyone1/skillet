use crate::profiles::McpRef;
use crate::util;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Clone)]
pub struct McpServer {
    pub name: String,
    pub endpoint: String,
    pub transport: String, // "http" | "stdio"
    pub status: String,    // "connected" | "needs-auth" | "pending" | "unknown"
    pub managed: bool,     // plugin: / claude.ai -> read-only
    pub scope: String,     // "user" | "local" | "project" | "plugin" | "managed" | "—"
    pub tools: Option<u32>,
}

/// Read layer = `claude mcp list` (captures plugin + managed + claude.ai servers
/// that the config files do not) enriched with scope read straight from config.
/// Per the build plan: never *write* the JSON, only read it.
pub fn list() -> Vec<McpServer> {
    let raw = match util::run_claude(&["mcp", "list"]) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let scopes = scope_map();
    let mut out = Vec::new();

    for line in raw.lines() {
        if let Some(srv) = parse_line(line, &scopes) {
            out.push(srv);
        }
    }
    out
}

/// Build the `mcpServers` object for a strict `--mcp-config` file, resolving each
/// selected `McpRef` to its full server spec for the given `workdir`. Only
/// isolatable scopes (user / local / project) can be reconstructed; plugin and
/// managed servers have no re-specifiable config, so they are reported as
/// unresolved (the UI hard-disables them, this is the defensive backstop).
///
/// Returns `(mcpServers, unresolved_names)`.
pub fn resolve_specs(workdir: &Path, refs: &[McpRef]) -> (Map<String, Value>, Vec<String>) {
    let (user, local, project) = spec_sources(workdir);
    let mut out = Map::new();
    let mut unresolved = Vec::new();

    for r in refs {
        // Strict: resolve ONLY within the ref's declared scope — no cross-scope
        // fallback (that could launch a different same-named server). A miss for
        // the launch workdir is an error, surfaced to the user.
        let picked = match r.scope.as_str() {
            "user" => user.get(&r.name),
            "local" => local.get(&r.name),
            "project" => project.get(&r.name),
            _ => None,
        };
        match picked {
            Some(spec) => {
                out.insert(r.name.clone(), spec.clone());
            }
            None => unresolved.push(r.name.clone()),
        }
    }
    (out, unresolved)
}

/// (user, local, project) maps of name -> full server spec, read (never written)
/// from the canonical config files for `workdir`.
fn spec_sources(workdir: &Path) -> (
    Map<String, Value>,
    Map<String, Value>,
    Map<String, Value>,
) {
    let mut user = Map::new();
    let mut local = Map::new();
    let mut project = Map::new();

    if let Ok(txt) = fs::read_to_string(util::home_dir().join(".claude.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            collect_specs(&v, &mut user);
            if let Some(projects) = v.get("projects").and_then(Value::as_object) {
                let key = workdir.to_string_lossy();
                if let Some(pv) = projects.get(key.as_ref()) {
                    collect_specs(pv, &mut local);
                }
            }
        }
    }
    if let Ok(txt) = fs::read_to_string(workdir.join(".mcp.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            collect_specs(&v, &mut project);
        }
    }
    (user, local, project)
}

fn collect_specs(v: &Value, out: &mut Map<String, Value>) {
    if let Some(obj) = v.get("mcpServers").and_then(Value::as_object) {
        for (k, spec) in obj {
            out.insert(k.clone(), spec.clone());
        }
    }
}

/// Line shape: `name: <command-or-url> [(TRANSPORT)] - <glyph> <status text>`
fn parse_line(line: &str, scopes: &HashMap<String, String>) -> Option<McpServer> {
    let line = line.trim();
    // health-check banner / blank lines
    if line.is_empty() || !line.contains(": ") {
        return None;
    }
    // split the status suffix off the right — but only when the tail actually
    // looks like a status, so a command/url containing " - " isn't mis-split.
    let (left, status_part) = match line.rsplit_once(" - ") {
        Some((l, r)) if looks_like_status(r.trim()) => (l.trim(), r.trim()),
        _ => (line, ""),
    };

    let (name, mut endpoint) = left.split_once(": ").map(|(n, e)| (n.trim(), e.trim()))?;
    let name = name.to_string();

    // strip a trailing "(HTTP)" / "(SSE)" transport annotation — but ONLY that,
    // so a command/url legitimately ending in "(...)" is left intact.
    let mut transport_hint: Option<&str> = None;
    if endpoint.ends_with(')') {
        if let Some(idx) = endpoint.rfind('(') {
            let hint = endpoint[idx + 1..endpoint.len() - 1].trim();
            if hint.eq_ignore_ascii_case("http") || hint.eq_ignore_ascii_case("sse") {
                transport_hint = Some(hint);
                endpoint = endpoint[..idx].trim();
            }
        }
    }

    let transport = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        "http"
    } else if matches!(transport_hint, Some(h) if h.eq_ignore_ascii_case("http") || h.eq_ignore_ascii_case("sse")) {
        "http"
    } else {
        "stdio"
    };

    let status = classify_status(status_part);

    let managed = name.starts_with("plugin:") || name.starts_with("claude.ai");
    let scope = if name.starts_with("plugin:") {
        "plugin".to_string()
    } else if name.starts_with("claude.ai") {
        "managed".to_string()
    } else {
        scopes.get(&name).cloned().unwrap_or_else(|| "—".into())
    };

    Some(McpServer {
        name,
        endpoint: endpoint.to_string(),
        transport: transport.into(),
        status,
        managed,
        scope,
        tools: None, // tool counts need a live session; not exposed by `mcp list`
    })
}

/// Does this right-hand fragment look like a status (glyph or known word),
/// rather than part of a command/url that happened to contain " - "?
fn looks_like_status(s: &str) -> bool {
    let s = s.trim();
    match s.chars().next() {
        None => false,
        Some(c) if "✔✓!⏸✗✘●○⚠".contains(c) => true,
        _ => classify_status(s) != "unknown",
    }
}

fn classify_status(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("connected") {
        "connected".into()
    } else if l.contains("auth") {
        "needs-auth".into()
    } else if l.contains("pending") {
        "pending".into()
    } else if l.contains("fail") || l.contains("error") {
        "error".into()
    } else {
        "unknown".into()
    }
}

/// name -> scope, read (never written) from the canonical config files.
fn scope_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let root = util::project_root();

    // ~/.claude.json : top-level mcpServers = user; projects.<root>.mcpServers = local.
    // Only the CURRENT project's entry, not every project (avoids labeling a
    // same-named server from an unrelated project as "local").
    if let Ok(txt) = fs::read_to_string(util::home_dir().join(".claude.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            insert_keys(&v, "mcpServers", "user", &mut map, true);
            if let (Some(projects), Some(root)) =
                (v.get("projects").and_then(Value::as_object), root.as_ref())
            {
                let key = root.to_string_lossy();
                if let Some(pv) = projects.get(key.as_ref()) {
                    insert_keys(pv, "mcpServers", "local", &mut map, false);
                }
            }
        }
    }

    // <project-root>/.mcp.json : project scope
    if let Some(root) = root.as_ref() {
        if let Ok(txt) = fs::read_to_string(root.join(".mcp.json")) {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                insert_keys(&v, "mcpServers", "project", &mut map, true);
            }
        }
    }

    map
}

fn insert_keys(v: &Value, key: &str, scope: &str, map: &mut HashMap<String, String>, overwrite: bool) {
    if let Some(obj) = v.get(key).and_then(Value::as_object) {
        for k in obj.keys() {
            if overwrite {
                map.insert(k.clone(), scope.into());
            } else {
                map.entry(k.clone()).or_insert_with(|| scope.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `claude mcp list` output (Claude Code 2.1.x, captured 2026-09-07),
    /// sanitized. Lines start at column 0 on purpose: the fixture is fed
    /// line-by-line exactly as the CLI prints it.
    const FIXTURE: &str = r"Checking MCP server health…

claude.ai Google Drive: https://drivemcp.googleapis.com/mcp/v1 - ✔ Connected
plugin:context7:context7: https://mcp.context7.com/mcp (HTTP) - ✔ Connected
plugin:playwright:playwright: npx @playwright/mcp@latest - ✔ Connected
example: https://mcp.example.com/ (HTTP) - ✘ Failed to connect — Protected resource https://mcp.example.com/mcp does not match expected https://mcp.example.com/ (or origin)
github: https://api.githubcopilot.com/mcp/ (HTTP) - ✔ Connected
local-tool: uv run --directory C:\Users\me\mcp-servers\demo demo-mcp - ✘ Failed to connect — CONNECTION_CLOSED: Connection closed
chrome-devtools: npx -y chrome-devtools-mcp@latest --browserUrl http://127.0.0.1:9222 - ✔ Connected
";

    /// Scope map as `scope_map()` would build it from the config files.
    fn scopes() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("github".to_string(), "user".to_string());
        m.insert("local-tool".to_string(), "local".to_string());
        m
    }

    fn parse(line: &str) -> McpServer {
        parse_line(line, &scopes()).unwrap_or_else(|| panic!("line should parse: {line}"))
    }

    fn parse_fixture() -> Vec<McpServer> {
        FIXTURE.lines().filter_map(|l| parse_line(l, &scopes())).collect()
    }

    fn find<'a>(servers: &'a [McpServer], name: &str) -> &'a McpServer {
        servers
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no server named {name}"))
    }

    #[test]
    fn header_and_blank_lines_are_ignored() {
        let s = scopes();
        assert!(parse_line("Checking MCP server health…", &s).is_none());
        assert!(parse_line("", &s).is_none());
        assert!(parse_line("   ", &s).is_none());

        let all = parse_fixture();
        let names: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "claude.ai Google Drive",
                "plugin:context7:context7",
                "plugin:playwright:playwright",
                "example",
                "github",
                "local-tool",
                "chrome-devtools",
            ]
        );
    }

    #[test]
    fn name_and_endpoint_split_on_first_colon_space() {
        // plugin names carry extra ':' with no space after — only ": " splits
        let m = parse("plugin:context7:context7: https://mcp.context7.com/mcp (HTTP) - ✔ Connected");
        assert_eq!(m.name, "plugin:context7:context7");
        assert_eq!(m.endpoint, "https://mcp.context7.com/mcp");

        // a name may contain spaces
        let m = parse("claude.ai Google Drive: https://drivemcp.googleapis.com/mcp/v1 - ✔ Connected");
        assert_eq!(m.name, "claude.ai Google Drive");
        assert_eq!(m.endpoint, "https://drivemcp.googleapis.com/mcp/v1");

        // a drive letter in the command and a ": " inside the status text are
        // not separators: status is split off first, then the first ": " wins
        let m = parse(r"local-tool: uv run --directory C:\Users\me\mcp-servers\demo demo-mcp - ✘ Failed to connect — CONNECTION_CLOSED: Connection closed");
        assert_eq!(m.name, "local-tool");
        assert_eq!(m.endpoint, r"uv run --directory C:\Users\me\mcp-servers\demo demo-mcp");
    }

    #[test]
    fn http_suffix_is_stripped_and_means_http_transport() {
        let all = parse_fixture();
        for (name, endpoint) in [
            ("plugin:context7:context7", "https://mcp.context7.com/mcp"),
            ("example", "https://mcp.example.com/"),
            ("github", "https://api.githubcopilot.com/mcp/"),
        ] {
            let m = find(&all, name);
            assert_eq!(m.endpoint, endpoint, "{name}");
            assert_eq!(m.transport, "http", "{name}");
        }

        // an https:// endpoint without the annotation is http as well
        let m = find(&all, "claude.ai Google Drive");
        assert_eq!(m.endpoint, "https://drivemcp.googleapis.com/mcp/v1");
        assert_eq!(m.transport, "http");

        // (SSE) is an http-family hint too; the hint is case-insensitive and
        // on its own is enough to make a non-URL endpoint http
        let m = parse("a: https://x.example/sse (SSE) - ✔ Connected");
        assert_eq!(m.endpoint, "https://x.example/sse");
        assert_eq!(m.transport, "http");
        let m = parse("b: my-gateway (http) - ✔ Connected");
        assert_eq!(m.endpoint, "my-gateway");
        assert_eq!(m.transport, "http");
    }

    #[test]
    fn only_transport_parentheticals_are_stripped() {
        let m = parse("dev: node server.js (dev) - ✔ Connected");
        assert_eq!(m.endpoint, "node server.js (dev)");
        assert_eq!(m.transport, "stdio");
    }

    #[test]
    fn stdio_commands_yield_stdio_transport() {
        let all = parse_fixture();
        let m = find(&all, "plugin:playwright:playwright");
        assert_eq!(m.endpoint, "npx @playwright/mcp@latest");
        assert_eq!(m.transport, "stdio");
        assert_eq!(find(&all, "local-tool").transport, "stdio");

        // a URL *inside* a command does not make it http; the full command survives
        let m = find(&all, "chrome-devtools");
        assert_eq!(
            m.endpoint,
            "npx -y chrome-devtools-mcp@latest --browserUrl http://127.0.0.1:9222"
        );
        assert_eq!(m.transport, "stdio");
    }

    #[test]
    fn dash_inside_command_does_not_break_the_status_split() {
        // status is split off the *right*, so " - " inside the command survives
        let m = parse("range: npx -y tool --range 1 - 5 - ✔ Connected");
        assert_eq!(m.endpoint, "npx -y tool --range 1 - 5");
        assert_eq!(m.status, "connected");

        // a trailing " - <not a status>" stays part of the command
        let m = parse("nostatus: npx tool - baz");
        assert_eq!(m.endpoint, "npx tool - baz");
        assert_eq!(m.status, "unknown");
    }

    #[test]
    fn fixture_statuses() {
        let all = parse_fixture();
        for name in [
            "claude.ai Google Drive",
            "plugin:context7:context7",
            "plugin:playwright:playwright",
            "github",
            "chrome-devtools",
        ] {
            assert_eq!(find(&all, name).status, "connected", "{name}");
        }
        for name in ["example", "local-tool"] {
            assert_eq!(find(&all, name).status, "error", "{name}");
        }
    }

    #[test]
    fn needs_auth_pending_and_unknown_statuses() {
        let m = parse("foo: https://x.example/mcp (HTTP) - ⚠ Needs authentication");
        assert_eq!(m.status, "needs-auth");
        assert_eq!(m.endpoint, "https://x.example/mcp");

        assert_eq!(parse("foo: https://x.example/mcp - ⏸ Pending").status, "pending");

        // no status suffix at all
        let m = parse("foo: https://x.example/mcp");
        assert_eq!(m.status, "unknown");
        assert_eq!(m.endpoint, "https://x.example/mcp");
    }

    #[test]
    fn classify_status_keywords() {
        assert_eq!(classify_status("✔ Connected"), "connected");
        assert_eq!(
            classify_status("✘ Failed to connect — CONNECTION_CLOSED: Connection closed"),
            "error"
        );
        assert_eq!(classify_status("Error: timeout"), "error");
        assert_eq!(classify_status("⚠ Needs authentication"), "needs-auth");
        assert_eq!(classify_status("Pending"), "pending");
        assert_eq!(classify_status(""), "unknown");
        assert_eq!(classify_status("something else"), "unknown");
    }

    #[test]
    fn looks_like_status_accepts_glyphs_or_known_words() {
        assert!(looks_like_status("✔ Connected"));
        assert!(looks_like_status("✘ Failed"));
        assert!(looks_like_status("⚠ Needs authentication"));
        assert!(looks_like_status("Connected"));
        assert!(looks_like_status("failed"));
        assert!(!looks_like_status(""));
        assert!(!looks_like_status("5"));
        assert!(!looks_like_status("baz"));
        assert!(!looks_like_status("--browserUrl http://127.0.0.1:9222"));
    }

    #[test]
    fn managed_prefixes_and_scopes() {
        let all = parse_fixture();
        for name in ["plugin:context7:context7", "plugin:playwright:playwright"] {
            let m = find(&all, name);
            assert!(m.managed, "{name}");
            assert_eq!(m.scope, "plugin", "{name}");
        }
        let m = find(&all, "claude.ai Google Drive");
        assert!(m.managed);
        assert_eq!(m.scope, "managed");

        // plain servers take their scope from the config map, "—" when absent
        let m = find(&all, "github");
        assert!(!m.managed);
        assert_eq!(m.scope, "user");
        assert_eq!(find(&all, "local-tool").scope, "local");
        assert_eq!(find(&all, "example").scope, "—");
        assert_eq!(find(&all, "chrome-devtools").scope, "—");

        // `mcp list` never reports tool counts
        assert!(all.iter().all(|m| m.tools.is_none()));
    }

    #[test]
    fn prefix_scope_beats_config_map() {
        let mut s = scopes();
        s.insert("plugin:context7:context7".into(), "user".into());
        s.insert("claude.ai Google Drive".into(), "project".into());
        let m = parse_line(
            "plugin:context7:context7: https://mcp.context7.com/mcp (HTTP) - ✔ Connected",
            &s,
        )
        .unwrap();
        assert_eq!(m.scope, "plugin");
        let m = parse_line(
            "claude.ai Google Drive: https://drivemcp.googleapis.com/mcp/v1 - ✔ Connected",
            &s,
        )
        .unwrap();
        assert_eq!(m.scope, "managed");
    }
}
