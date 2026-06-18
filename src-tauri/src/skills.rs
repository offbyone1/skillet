use crate::util;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Serialize, Clone)]
pub struct Skill {
    pub name: String,
    pub scope: String,          // "personal" | "project" | "plugin"
    pub plugin: Option<String>, // owning plugin name for scope "plugin", else None
    pub enabled: bool,
    pub description: String,
    pub allowed_tools: Option<String>,
    pub size_bytes: u64,
    pub size_human: String,
    pub file_count: u32,
    pub modified: String,
    pub modified_ts: u64,
    pub desc_quality: String, // "good" | "warn" | "bad"
    pub path: String,
}

/// Scan every scope and return the full skill inventory.
pub fn scan_all() -> Vec<Skill> {
    let mut out = Vec::new();
    let home = util::home_dir();

    // personal: ~/.claude/skills/*
    scan_skills_root(&home.join(".claude").join("skills"), "personal", None, &mut out);

    // project: <project-root>/.claude/skills/* (root resolved by walking up, not
    // the raw process cwd — a packaged app may launch outside the project)
    if let Some(root) = util::project_root() {
        scan_skills_root(&root.join(".claude").join("skills"), "project", None, &mut out);
    }

    // plugin: authoritative — read installed_plugins.json and scan each plugin's
    // own `skills/` dir. A blind FS-walk of ~/.claude/plugins counted every skill
    // 2-4× (the same plugin lives under cache/ and marketplaces/, each nested),
    // which the manifest avoids: one record per installed plugin.
    for (plugin, install_path) in installed_plugins(&home) {
        scan_skills_root(&install_path.join("skills"), "plugin", Some(&plugin), &mut out);
    }

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Parse `~/.claude/plugins/installed_plugins.json` into (plugin_name, install_path).
/// Keys look like `"caveman@caveman"`; the segment before `@` is the plugin's
/// display name. Each value is an array of install records — we take the first
/// one that carries an `installPath`.
fn installed_plugins(home: &Path) -> Vec<(String, PathBuf)> {
    let manifest = home
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json");
    let content = match fs::read_to_string(&manifest) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Some(map) = json.get("plugins").and_then(|p| p.as_object()) {
        for (key, installs) in map {
            let name = key.split('@').next().unwrap_or(key).to_string();
            let path = installs
                .as_array()
                .and_then(|arr| {
                    arr.iter()
                        .find_map(|i| i.get("installPath").and_then(|p| p.as_str()))
                })
                .map(PathBuf::from);
            if let Some(path) = path {
                out.push((name, path));
            }
        }
    }
    out
}

/// Each immediate subdirectory of `root` that contains a SKILL.md is one skill.
fn scan_skills_root(root: &Path, scope: &str, plugin: Option<&str>, out: &mut Vec<Skill>) {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let skill_md = dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let content = fs::read_to_string(&skill_md).unwrap_or_default();
        let (fm_name, fm_desc, fm_tools) = parse_frontmatter(&content);

        let name = fm_name.unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into())
        });
        let description = fm_desc.unwrap_or_default();
        let (size_bytes, file_count, newest) = dir_stats(&dir);

        out.push(Skill {
            desc_quality: desc_quality(&description).into(),
            name,
            scope: scope.into(),
            plugin: plugin.map(|p| p.to_string()),
            enabled: true, // present in scan path == enabled (M0 read-only)
            description,
            allowed_tools: fm_tools,
            size_bytes,
            size_human: util::human_size(size_bytes),
            file_count,
            modified: util::rel_time(newest),
            modified_ts: newest,
            path: dir.to_string_lossy().to_string(),
        });
    }
}

/// (total bytes, file count, newest mtime as epoch secs) over a directory tree.
fn dir_stats(dir: &Path) -> (u64, u32, u64) {
    dir_stats_depth(dir, 0)
}

/// Depth-capped so a symlink/junction cycle can't recurse into a stack overflow.
/// `entry.metadata()` returns symlink-level metadata, so plain symlinks aren't
/// followed; the cap guards the Windows-junction edge case.
fn dir_stats_depth(dir: &Path, depth: u32) -> (u64, u32, u64) {
    let mut size = 0u64;
    let mut count = 0u32;
    let mut newest = 0u64;
    if depth > 24 {
        return (0, 0, 0);
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (0, 0, 0),
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.is_dir() {
            let (s, c, n) = dir_stats_depth(&p, depth + 1);
            size += s;
            count += c;
            if n > newest {
                newest = n;
            }
        } else {
            size += md.len();
            count += 1;
            if let Ok(m) = md.modified() {
                if let Ok(d) = m.duration_since(UNIX_EPOCH) {
                    let t = d.as_secs();
                    if t > newest {
                        newest = t;
                    }
                }
            }
        }
    }
    (size, count, newest)
}

/// Pull name / description / allowed-tools from leading `---` YAML frontmatter.
/// Parses the block as real YAML (handles quotes, `>-`/`|` block scalars and
/// list-valued allowed-tools), falling back to a line scan if YAML is malformed.
fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>, Option<String>) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, None, None);
    }
    let after = &trimmed[3..];
    let end = match after.find("\n---") {
        Some(e) => e,
        None => return (None, None, None),
    };
    let block = &after[..end];

    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(block) {
        let name = yaml_string(&val, "name");
        let desc = yaml_string(&val, "description");
        let tools = yaml_joined(&val, "allowed-tools");
        if name.is_some() || desc.is_some() || tools.is_some() {
            return (name, desc, tools);
        }
    }
    parse_frontmatter_lines(block)
}

fn yaml_string(v: &serde_yaml::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// allowed-tools may be a comma string or a YAML list; normalize to one string.
fn yaml_joined(v: &serde_yaml::Value, key: &str) -> Option<String> {
    match v.get(key) {
        Some(serde_yaml::Value::String(s)) => {
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        }
        Some(serde_yaml::Value::Sequence(seq)) => {
            let items: Vec<String> = seq
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            (!items.is_empty()).then(|| items.join(", "))
        }
        _ => None,
    }
}

/// Fallback for malformed YAML: single-line `key: value` scan.
fn parse_frontmatter_lines(block: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut name = None;
    let mut desc = None;
    let mut tools = None;
    for line in block.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("description:") {
            desc = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("allowed-tools:") {
            tools = Some(unquote(v));
        }
    }
    (name, desc, tools)
}

fn unquote(s: &str) -> String {
    s.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

/// Cheap description linter: does it signal *when* the skill triggers?
fn desc_quality(desc: &str) -> &'static str {
    let d = desc.trim();
    let words = d.split_whitespace().count();
    let lower = d.to_lowercase();
    let has_trigger = lower.contains("when")
        || lower.contains("use ")
        || lower.contains("trigger")
        || lower.contains("/")
        || lower.contains(".md")
        || lower.contains("file");

    if d.len() < 25 || words < 4 {
        "bad"
    } else if d.len() < 60 || !has_trigger {
        "warn"
    } else {
        "good"
    }
}
