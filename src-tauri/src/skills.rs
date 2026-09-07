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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;

    /// Unique scratch dir under the OS temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = env::temp_dir().join(format!(
                "skillet_test_skills_{tag}_{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Create `<root>/<dir>/SKILL.md` with the given content; returns the skill dir.
    fn write_skill(root: &Path, dir: &str, skill_md: &str) -> PathBuf {
        let d = root.join(dir);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), skill_md).unwrap();
        d
    }

    fn write_manifest(home: &Path, manifest: serde_json::Value) {
        let dir = home.join(".claude").join("plugins");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("installed_plugins.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    /// One install record in the version-2 `installed_plugins.json` shape.
    fn record(install: &Path, version: &str) -> serde_json::Value {
        json!({
            "scope": "user",
            "installPath": install.to_string_lossy(),
            "version": version,
            "installedAt": "2026-08-01T10:00:00.000Z",
            "lastUpdated": "2026-09-01T10:00:00.000Z",
            "gitCommitSha": "0123456789abcdef"
        })
    }

    // --- frontmatter ---

    #[test]
    fn frontmatter_parses_name_description_and_tools() {
        let md = "---\nname: verify\ndescription: \"Runs the checks. Use when the user asks to verify.\"\nallowed-tools: Bash, Read, Grep\n---\n# Verify\n\nBody text.\n";
        let (name, desc, tools) = parse_frontmatter(md);
        assert_eq!(name.as_deref(), Some("verify"));
        assert_eq!(desc.as_deref(), Some("Runs the checks. Use when the user asks to verify."));
        assert_eq!(tools.as_deref(), Some("Bash, Read, Grep"));
    }

    #[test]
    fn frontmatter_folded_multiline_description() {
        let md = "---\nname: deploy\ndescription: >-\n  Deploys the app to staging.\n  Use when the user asks to ship a build.\n---\n# Deploy\n";
        let (name, desc, _) = parse_frontmatter(md);
        assert_eq!(name.as_deref(), Some("deploy"));
        assert_eq!(
            desc.as_deref(),
            Some("Deploys the app to staging. Use when the user asks to ship a build.")
        );
    }

    #[test]
    fn frontmatter_literal_block_description_keeps_newlines() {
        let md = "---\nname: x\ndescription: |\n  Line one.\n  Line two.\n---\n";
        let (_, desc, _) = parse_frontmatter(md);
        assert_eq!(desc.as_deref(), Some("Line one.\nLine two."));
    }

    #[test]
    fn frontmatter_allowed_tools_list_is_joined() {
        let md = "---\nname: x\nallowed-tools:\n  - Bash\n  - Read\n---\n";
        let (_, _, tools) = parse_frontmatter(md);
        assert_eq!(tools.as_deref(), Some("Bash, Read"));
    }

    #[test]
    fn frontmatter_missing_description() {
        let (name, desc, tools) = parse_frontmatter("---\nname: foo\n---\n# Foo\n");
        assert_eq!(name.as_deref(), Some("foo"));
        assert_eq!(desc, None);
        assert_eq!(tools, None);

        // an empty value counts as missing
        let (name, desc, _) = parse_frontmatter("---\nname: foo\ndescription: \"\"\n---\n");
        assert_eq!(name.as_deref(), Some("foo"));
        assert_eq!(desc, None);
    }

    #[test]
    fn frontmatter_absent_or_unterminated() {
        assert_eq!(
            parse_frontmatter("# Just a heading\n\nname: not-frontmatter\n"),
            (None, None, None)
        );
        assert_eq!(parse_frontmatter(""), (None, None, None));
        assert_eq!(
            parse_frontmatter("---\nname: foo\nno closing fence\n"),
            (None, None, None)
        );
    }

    #[test]
    fn frontmatter_tolerates_leading_blank_lines() {
        let (name, desc, _) =
            parse_frontmatter("\n\n---\nname: x\ndescription: \"Alpha. Use when asked for alpha.\"\n---\n");
        assert_eq!(name.as_deref(), Some("x"));
        assert_eq!(desc.as_deref(), Some("Alpha. Use when asked for alpha."));
    }

    #[test]
    fn frontmatter_tolerates_crlf() {
        let md = "---\r\nname: x\r\ndescription: \"Alpha. Use when asked for alpha.\"\r\nallowed-tools: Bash\r\n---\r\nbody\r\n";
        let (name, desc, tools) = parse_frontmatter(md);
        assert_eq!(name.as_deref(), Some("x"));
        assert_eq!(desc.as_deref(), Some("Alpha. Use when asked for alpha."));
        assert_eq!(tools.as_deref(), Some("Bash"));
    }

    #[test]
    fn frontmatter_malformed_yaml_falls_back_to_line_scan() {
        // `a: b: c` is a YAML scanner error, so the whole block fails to parse
        let md = "---\nname: lint\ndescription: 'Lints files. Use when asked to lint.'\nweird: this: is: broken\n---\n";
        let (name, desc, tools) = parse_frontmatter(md);
        assert_eq!(name.as_deref(), Some("lint"));
        assert_eq!(desc.as_deref(), Some("Lints files. Use when asked to lint."));
        assert_eq!(tools, None);
    }

    #[test]
    fn unquote_strips_surrounding_quotes_and_space() {
        assert_eq!(unquote("  \"a b\"  "), "a b");
        assert_eq!(unquote("'x'"), "x");
        assert_eq!(unquote(" plain "), "plain");
        assert_eq!(unquote(""), "");
    }

    // --- description lint ---
    // Thresholds: len < 25 or < 4 words -> bad; len < 60 or no trigger term -> warn.

    #[test]
    fn desc_quality_bad_when_short_or_few_words() {
        assert_eq!(desc_quality(""), "bad");
        assert_eq!(desc_quality("   "), "bad");
        assert_eq!(desc_quality("Does stuff"), "bad");

        // 24 chars: one short of the floor even though it has 5 words
        let s = "Runs the unit test suite";
        assert_eq!(s.len(), 24);
        assert_eq!(desc_quality(s), "bad");

        // long enough, but fewer than 4 words
        let s = "Supercalifragilisticexpialidocious documentation";
        assert!(s.len() >= 25);
        assert_eq!(desc_quality(s), "bad");
        let s = "Documentation generator tool";
        assert!(s.len() >= 25);
        assert_eq!(desc_quality(s), "bad");
    }

    #[test]
    fn desc_quality_warn_when_medium_or_no_trigger() {
        // 25 chars / 5 words: clears the floor, still under 60
        let s = "Runs the whole test suite";
        assert_eq!(s.len(), 25);
        assert_eq!(desc_quality(s), "warn");

        // trigger term present but under 60 chars
        let s = "Use when asked to deploy the app.";
        assert!(s.len() < 60);
        assert_eq!(desc_quality(s), "warn");

        // 60+ chars but nothing says when it fires
        let s = "Generates comprehensive documentation for every public function and type in the crate.";
        assert!(s.len() >= 60);
        assert_eq!(desc_quality(s), "warn");

        // 59 chars with a trigger term: one short of good
        let s = "Use when the user asks to run the full regression test suit";
        assert_eq!(s.len(), 59);
        assert_eq!(desc_quality(s), "warn");
    }

    #[test]
    fn desc_quality_good_when_long_with_trigger() {
        let s = "Use when the user asks to run the full regression test suite";
        assert_eq!(s.len(), 60);
        assert_eq!(desc_quality(s), "good");

        assert_eq!(
            desc_quality("Runs the full test suite and reports failures. Use when the user asks to verify a change."),
            "good"
        );

        // trigger terms are matched case-insensitively
        assert_eq!(
            desc_quality("Deploys builds to staging. USE WHEN the release manager asks for a deploy."),
            "good"
        );

        // a slash command or a .md reference also counts as a trigger
        let s = "Formats every source document in the repository, invoked via the /fmt command";
        assert!(s.len() >= 60);
        assert_eq!(desc_quality(s), "good");
        let s = "Rewrites the CLAUDE.md memory document into compressed caveman format for the repo";
        assert!(s.len() >= 60);
        assert_eq!(desc_quality(s), "good");
    }

    // --- installed_plugins.json ---

    #[test]
    fn installed_plugins_parses_v2_manifest() {
        let tmp = TempDir::new("plugins_v2");
        let home = tmp.path().join("home");
        let cave = tmp.path().join("cache").join("caveman").join("caveman").join("f00d");
        let c7 = tmp
            .path()
            .join("cache")
            .join("claude-plugins-official")
            .join("context7")
            .join("beef");
        write_manifest(
            &home,
            json!({
                "version": 2,
                "plugins": {
                    "caveman@caveman": [record(&cave, "1.2.0")],
                    // two install records -> still one plugin
                    "context7@claude-plugins-official": [record(&c7, "2.0.0"), record(&c7, "2.1.0")]
                }
            }),
        );

        let mut got = installed_plugins(&home);
        got.sort();
        assert_eq!(
            got,
            vec![("caveman".to_string(), cave), ("context7".to_string(), c7)]
        );
    }

    #[test]
    fn installed_plugins_takes_first_record_with_install_path() {
        let tmp = TempDir::new("plugins_first_path");
        let home = tmp.path().join("home");
        let p = tmp.path().join("cache").join("bar").join("foo").join("c0de");
        write_manifest(
            &home,
            json!({
                "version": 2,
                "plugins": {
                    "foo@bar": [{"scope": "user", "version": "0.1.0"}, record(&p, "0.2.0")],
                    "nopath@bar": [{"scope": "user", "version": "0.1.0"}]
                }
            }),
        );
        assert_eq!(installed_plugins(&home), vec![("foo".to_string(), p)]);
    }

    #[test]
    fn installed_plugins_missing_or_corrupt_manifest_is_empty() {
        let tmp = TempDir::new("plugins_missing");
        assert!(installed_plugins(tmp.path()).is_empty());

        let home = tmp.path().join("home");
        let manifest = home.join(".claude").join("plugins").join("installed_plugins.json");
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, "{not json").unwrap();
        assert!(installed_plugins(&home).is_empty());

        // valid JSON, but no `plugins` object
        fs::write(&manifest, r#"{"version": 2}"#).unwrap();
        assert!(installed_plugins(&home).is_empty());
    }

    // --- directory scan ---

    #[test]
    fn scan_skills_root_one_skill_per_dir_with_skill_md() {
        let tmp = TempDir::new("scan_root");
        let root = tmp.path();
        write_skill(
            root,
            "alpha",
            "---\nname: alpha-skill\ndescription: \"Alpha. Use when the user asks for the alpha workflow.\"\n---\n# Alpha\n",
        );
        write_skill(root, "gamma", "# No frontmatter at all\n");
        fs::create_dir_all(root.join("beta")).unwrap(); // dir without SKILL.md
        fs::write(root.join("README.md"), "not a skill").unwrap(); // stray file

        let mut out = Vec::new();
        scan_skills_root(root, "personal", None, &mut out);
        out.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "alpha-skill");
        assert_eq!(out[1].name, "gamma"); // falls back to the directory name
        assert_eq!(out[1].description, "");
        assert_eq!(out[1].desc_quality, "bad");
        assert!(out.iter().all(|s| s.scope == "personal" && s.plugin.is_none()));
    }

    #[test]
    fn scan_skills_root_fills_stats_and_quality() {
        let tmp = TempDir::new("scan_stats");
        let root = tmp.path();
        let md = "---\nname: verify\ndescription: Runs the checks. Use when the user asks to verify a change before commit.\nallowed-tools: Bash, Read\n---\n# Verify\n";
        let dir = write_skill(root, "verify", md);
        let extra = "helper script";
        fs::write(dir.join("run.sh"), extra).unwrap();

        let mut out = Vec::new();
        scan_skills_root(root, "project", None, &mut out);
        assert_eq!(out.len(), 1);
        let s = &out[0];
        assert_eq!(s.name, "verify");
        assert_eq!(s.scope, "project");
        assert_eq!(
            s.description,
            "Runs the checks. Use when the user asks to verify a change before commit."
        );
        assert_eq!(s.desc_quality, "good");
        assert_eq!(s.allowed_tools.as_deref(), Some("Bash, Read"));
        assert_eq!(s.file_count, 2);
        assert_eq!(s.size_bytes, (md.len() + extra.len()) as u64);
        assert_eq!(s.size_human, format!("{} B", s.size_bytes));
        assert!(s.modified_ts > 0);
        assert_ne!(s.modified, "—");
        assert_eq!(Path::new(&s.path), dir);
    }

    #[test]
    fn scan_skills_root_missing_root_is_noop() {
        let tmp = TempDir::new("scan_missing");
        let mut out = Vec::new();
        scan_skills_root(&tmp.path().join("does-not-exist"), "personal", None, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn plugin_skills_are_scanned_once_per_installed_plugin() {
        let tmp = TempDir::new("plugins_e2e");
        let home = tmp.path().join("home");
        let install = tmp.path().join("cache").join("caveman").join("caveman").join("abc123");
        write_skill(
            &install.join("skills"),
            "caveman",
            "---\nname: caveman\ndescription: Talk like caveman. Use when the user asks for fewer tokens or says caveman mode.\n---\n",
        );
        write_manifest(
            &home,
            json!({
                "version": 2,
                "plugins": {
                    "caveman@caveman": [record(&install, "1.0.0"), record(&install, "1.1.0")]
                }
            }),
        );

        let plugins = installed_plugins(&home);
        assert_eq!(plugins.len(), 1);
        let mut out = Vec::new();
        for (plugin, path) in &plugins {
            scan_skills_root(&path.join("skills"), "plugin", Some(plugin), &mut out);
        }
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "caveman");
        assert_eq!(out[0].scope, "plugin");
        assert_eq!(out[0].plugin.as_deref(), Some("caveman"));
        assert_eq!(out[0].desc_quality, "good");
    }
}
