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
        // Prefer the ref's own scope; fall back to any source that has the name.
        let picked = match r.scope.as_str() {
            "user" => user.get(&r.name),
            "local" => local.get(&r.name),
            "project" => project.get(&r.name),
            _ => None,
        }
        .or_else(|| user.get(&r.name))
        .or_else(|| local.get(&r.name))
        .or_else(|| project.get(&r.name));

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
