use crate::mcp::McpServer;
use crate::skills::Skill;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct Finding {
    pub severity: String, // "crit" | "warn" | "info"
    pub title: String,
    pub detail: String,
    pub fix: String,
}

const FAT_SKILL_BYTES: u64 = 1024 * 1024; // 1 MB

/// M0 health pass: name collisions, weak descriptions, fat skills.
/// Drift (needs a DB baseline) and duplicate MCP tool names (needs live tool
/// lists) are out of scope until M5 — intentionally not reported here.
pub fn analyze(skills: &[Skill], _mcp: &[McpServer]) -> Vec<Finding> {
    let mut out = Vec::new();

    // name collisions across scopes
    let mut by_name: HashMap<&str, Vec<&str>> = HashMap::new();
    for s in skills {
        by_name.entry(&s.name).or_default().push(&s.scope);
    }
    for (name, mut scopes) in by_name {
        if scopes.len() > 1 {
            scopes.sort();
            scopes.dedup();
            if scopes.len() > 1 {
                let winner = if scopes.contains(&"project") {
                    "project"
                } else if scopes.contains(&"personal") {
                    "personal"
                } else {
                    "plugin"
                };
                out.push(Finding {
                    severity: "crit".into(),
                    title: format!("Skill name collision — {}", name),
                    detail: format!(
                        "Defined in {} scopes ({}). '{}' wins, the rest are ignored.",
                        scopes.len(),
                        scopes.join(", "),
                        winner
                    ),
                    fix: "inspect".into(),
                });
            }
        }
    }

    // weak descriptions
    for s in skills {
        if s.desc_quality == "bad" {
            out.push(Finding {
                severity: "warn".into(),
                title: format!("Weak description — {}", s.name),
                detail: format!(
                    "\"{}\" — too short / no trigger terms. Skill may never auto-fire.",
                    truncate(&s.description, 50)
                ),
                fix: "open linter".into(),
            });
        }
    }

    // fat skills
    for s in skills {
        if s.size_bytes > FAT_SKILL_BYTES {
            out.push(Finding {
                severity: "info".into(),
                title: format!("Fat skill — {}", s.name),
                detail: format!(
                    "{} across {} files. Review what ships.",
                    s.size_human, s.file_count
                ),
                fix: "breakdown".into(),
            });
        }
    }

    // stable order: crit, warn, info
    out.sort_by_key(|f| match f.severity.as_str() {
        "crit" => 0,
        "warn" => 1,
        _ => 2,
    });
    out
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{}…", t)
    }
}
