use crate::util;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A saved agent loadout. Stored in Skillet's own dir (`~/.skillet/profiles.json`),
/// never in `~/.claude`. See docs/superpowers/specs/2026-06-18-agent-starter-design.md.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Loadout {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub agent: String, // "claude" | "codex"
    #[serde(default)]
    pub workdir: String,
    #[serde(default)]
    pub skills: Vec<SkillRef>,
    #[serde(default)]
    pub mcps: Vec<McpRef>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub skip_perms: bool,
    #[serde(default)]
    pub builtin: bool,
}

/// Stable skill identity. Names alone collide across scopes/plugins, so we carry
/// scope + plugin + canonical path. `path` may be "" for builtin templates, which
/// resolve by (scope, name) against the live skill scan at launch.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRef {
    pub name: String,
    pub scope: String, // "personal" | "project" | "plugin"
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub path: String,
}

/// Stable MCP server identity. Names collide across user/local/project scopes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpRef {
    pub name: String,
    pub scope: String, // "user" | "local" | "project" | "plugin" | "managed"
    #[serde(default)]
    pub source_path: String,
}

/// `~/.skillet` — Skillet's own data dir (distinct from `~/.claude`).
pub fn skillet_dir() -> PathBuf {
    util::home_dir().join(".skillet")
}

fn profiles_path() -> PathBuf {
    skillet_dir().join("profiles.json")
}

/// Full list shown in the UI: builtin templates first, then saved loadouts.
pub fn list() -> Vec<Loadout> {
    let mut out = builtin_templates();
    out.extend(load_from(&profiles_path()));
    out
}

pub fn save(p: Loadout) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(skillet_dir()) {
        return Err(format!("create ~/.skillet: {e}"));
    }
    save_to(&profiles_path(), p)
}

pub fn delete(id: &str) -> Result<(), String> {
    delete_from(&profiles_path(), id)
}

// ----- testable core (explicit path, no real home) -----

/// Read the saved-loadouts array. Missing/invalid file -> empty (never errors;
/// a corrupt file must not brick the UI).
fn load_from(path: &PathBuf) -> Vec<Loadout> {
    let txt = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<Vec<Loadout>>(&txt).unwrap_or_default()
}

fn save_to(path: &PathBuf, mut p: Loadout) -> Result<(), String> {
    validate(&p)?;
    p.builtin = false; // saved loadouts are never builtin
    let mut all = load_from(path);
    match all.iter().position(|x| x.id == p.id) {
        Some(i) => all[i] = p,
        None => all.push(p),
    }
    write_all(path, &all)
}

fn delete_from(path: &PathBuf, id: &str) -> Result<(), String> {
    let mut all = load_from(path);
    let before = all.len();
    all.retain(|x| x.id != id);
    if all.len() == before {
        return Err(format!("profile not found: {id}"));
    }
    write_all(path, &all)
}

fn write_all(path: &PathBuf, all: &[Loadout]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("write {}: {e}", path.display()))
}

fn validate(p: &Loadout) -> Result<(), String> {
    if p.id.trim().is_empty() {
        return Err("profile id is empty".into());
    }
    if p.name.trim().is_empty() {
        return Err("profile name is empty".into());
    }
    if p.agent != "claude" && p.agent != "codex" {
        return Err(format!("unknown agent: {}", p.agent));
    }
    Ok(())
}

/// Shipped starter templates (read-only; "duplicate" to edit). Skill/MCP refs use
/// path="" / source_path="" and resolve by name at launch against the live scan.
pub fn builtin_templates() -> Vec<Loadout> {
    let skill = |name: &str, scope: &str, plugin: Option<&str>| SkillRef {
        name: name.into(),
        scope: scope.into(),
        plugin: plugin.map(|p| p.into()),
        path: String::new(),
    };
    let mcp = |name: &str, scope: &str| McpRef {
        name: name.into(),
        scope: scope.into(),
        source_path: String::new(),
    };
    vec![
        Loadout {
            id: "tmpl-research".into(),
            name: "Research".into(),
            note: "Tiefenrecherche mit Second-Opinion.".into(),
            agent: "claude".into(),
            workdir: String::new(),
            skills: vec![
                skill("deep-research", "personal", None),
                skill("consult-codex", "personal", None),
            ],
            mcps: vec![mcp("github", "user"), mcp("microsoft-learn", "user")],
            model: Some("opus".into()),
            effort: Some("high".into()),
            prompt: None,
            skip_perms: false,
            builtin: true,
        },
        Loadout {
            id: "tmpl-frontend".into(),
            name: "Frontend".into(),
            note: "UI bauen mit Design-Taste + Browser-Check.".into(),
            agent: "claude".into(),
            workdir: String::new(),
            skills: vec![
                skill("design-taste-frontend", "plugin", Some("design-taste-frontend")),
                skill("frontend-design", "plugin", Some("frontend-design")),
            ],
            mcps: vec![mcp("playwright", "user")],
            model: Some("sonnet".into()),
            effort: None,
            prompt: None,
            skip_perms: false,
            builtin: true,
        },
        Loadout {
            id: "tmpl-debug".into(),
            name: "Debug".into(),
            note: "Systematisches Debugging.".into(),
            agent: "claude".into(),
            workdir: String::new(),
            skills: vec![
                skill("systematic-debugging", "plugin", Some("superpowers")),
                skill("bugfix", "personal", None),
            ],
            mcps: vec![],
            model: None,
            effort: Some("high".into()),
            prompt: None,
            skip_perms: false,
            builtin: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp_path(tag: &str) -> PathBuf {
        let mut p = env::temp_dir();
        p.push(format!("skillet_test_profiles_{tag}_{}.json", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    fn sample(id: &str) -> Loadout {
        Loadout {
            id: id.into(),
            name: "My Loadout".into(),
            note: String::new(),
            agent: "claude".into(),
            workdir: "C:/proj".into(),
            skills: vec![SkillRef {
                name: "verify".into(),
                scope: "personal".into(),
                plugin: None,
                path: "C:/Users/x/.claude/skills/verify".into(),
            }],
            mcps: vec![],
            model: Some("opus".into()),
            effort: None,
            prompt: None,
            skip_perms: false,
            builtin: false,
        }
    }

    #[test]
    fn load_missing_file_is_empty() {
        let p = tmp_path("missing");
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let p = tmp_path("roundtrip");
        save_to(&p, sample("a1")).unwrap();
        let got = load_from(&p);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], sample("a1"));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn save_same_id_upserts_not_duplicates() {
        let p = tmp_path("upsert");
        save_to(&p, sample("a1")).unwrap();
        let mut edited = sample("a1");
        edited.name = "Renamed".into();
        save_to(&p, edited).unwrap();
        let got = load_from(&p);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "Renamed");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn save_forces_builtin_false() {
        let p = tmp_path("nobuiltin");
        let mut s = sample("a1");
        s.builtin = true;
        save_to(&p, s).unwrap();
        assert!(!load_from(&p)[0].builtin);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn delete_removes_and_errors_when_absent() {
        let p = tmp_path("delete");
        save_to(&p, sample("a1")).unwrap();
        delete_from(&p, "a1").unwrap();
        assert!(load_from(&p).is_empty());
        assert!(delete_from(&p, "a1").is_err());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn validate_rejects_bad_input() {
        let mut s = sample("");
        assert!(validate(&s).is_err());
        s = sample("a1");
        s.name = "  ".into();
        assert!(validate(&s).is_err());
        s = sample("a1");
        s.agent = "gemini".into();
        assert!(validate(&s).is_err());
        assert!(validate(&sample("a1")).is_ok());
    }

    #[test]
    fn corrupt_file_loads_empty() {
        let p = tmp_path("corrupt");
        fs::write(&p, "{not json").unwrap();
        assert!(load_from(&p).is_empty());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn builtin_templates_are_valid_and_marked() {
        for t in builtin_templates() {
            assert!(t.builtin);
            assert!(validate(&t).is_ok());
        }
    }
}
