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

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal, valid skill. Every field is explicit so this helper is the only
    /// place to touch when the struct changes.
    fn skill(name: &str, scope: &str, description: &str, desc_quality: &str) -> Skill {
        Skill {
            name: name.into(),
            scope: scope.into(),
            plugin: (scope == "plugin").then(|| "some-plugin".to_string()),
            description: description.into(),
            allowed_tools: None,
            size_bytes: 2 * 1024,
            size_human: "2 KB".into(),
            file_count: 1,
            modified: "1d".into(),
            modified_ts: 1_700_000_000,
            desc_quality: desc_quality.into(),
            path: format!("C:/skills/{scope}/{name}"),
        }
    }

    /// A skill that trips none of the checks.
    fn good(name: &str, scope: &str) -> Skill {
        skill(
            name,
            scope,
            "Runs the checks. Use when the user asks to verify a change.",
            "good",
        )
    }

    fn mcp(name: &str, status: &str) -> McpServer {
        McpServer {
            name: name.into(),
            endpoint: "https://mcp.example.com/mcp".into(),
            transport: "http".into(),
            status: status.into(),
            managed: false,
            scope: "user".into(),
            tools: None,
        }
    }

    fn titles(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.title.as_str()).collect()
    }

    #[test]
    fn clean_setup_yields_no_findings() {
        let skills = vec![
            good("verify", "personal"),
            good("deploy", "project"),
            good("caveman", "plugin"),
        ];
        let mcp = vec![mcp("github", "connected"), mcp("context7", "connected")];
        assert!(analyze(&skills, &mcp).is_empty());
        assert!(analyze(&[], &[]).is_empty());
    }

    #[test]
    fn duplicate_skill_name_across_scopes_is_crit() {
        let skills = vec![good("verify", "personal"), good("verify", "project")];
        let out = analyze(&skills, &[]);
        assert_eq!(out.len(), 1);
        let f = &out[0];
        assert_eq!(f.severity, "crit");
        assert_eq!(f.title, "Skill name collision — verify");
        assert_eq!(
            f.detail,
            "Defined in 2 scopes (personal, project). 'project' wins, the rest are ignored."
        );
        assert_eq!(f.fix, "inspect");
    }

    #[test]
    fn collision_winner_prefers_project_then_personal() {
        let out = analyze(&[good("x", "personal"), good("x", "plugin")], &[]);
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("'personal' wins"), "{}", out[0].detail);

        let out = analyze(
            &[good("x", "plugin"), good("x", "project"), good("x", "personal")],
            &[],
        );
        assert_eq!(out.len(), 1);
        assert!(
            out[0]
                .detail
                .starts_with("Defined in 3 scopes (personal, plugin, project)."),
            "{}",
            out[0].detail
        );
        assert!(out[0].detail.contains("'project' wins"));
    }

    #[test]
    fn same_name_within_one_scope_is_not_a_collision() {
        // two plugins shipping a same-named skill dedup to one scope -> silent
        let out = analyze(&[good("commit", "plugin"), good("commit", "plugin")], &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn bad_description_is_warn() {
        let skills = vec![
            skill("stub", "personal", "Does stuff", "bad"),
            skill("meh", "project", "Runs the whole test suite", "warn"),
            good("fine", "personal"),
        ];
        let out = analyze(&skills, &[]);
        assert_eq!(out.len(), 1);
        let f = &out[0];
        assert_eq!(f.severity, "warn");
        assert_eq!(f.title, "Weak description — stub");
        assert_eq!(
            f.detail,
            "\"Does stuff\" — too short / no trigger terms. Skill may never auto-fire."
        );
        assert_eq!(f.fix, "open linter");
    }

    #[test]
    fn weak_description_detail_is_truncated_to_50_chars() {
        let long = "a".repeat(80);
        let out = analyze(&[skill("verbose", "personal", &long, "bad")], &[]);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].detail.starts_with(&format!("\"{}…\" — ", "a".repeat(50))),
            "{}",
            out[0].detail
        );

        // an empty description still produces a finding
        let out = analyze(&[skill("empty", "personal", "", "bad")], &[]);
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.starts_with("\"\" — too short"));
    }

    #[test]
    fn fat_skill_is_info_above_one_megabyte() {
        let mut fat = good("huge", "personal");
        fat.size_bytes = FAT_SKILL_BYTES + 1;
        fat.size_human = "1.0 MB".into();
        fat.file_count = 12;
        let out = analyze(&[fat], &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, "info");
        assert_eq!(out[0].title, "Fat skill — huge");
        assert_eq!(out[0].detail, "1.0 MB across 12 files. Review what ships.");
        assert_eq!(out[0].fix, "breakdown");

        // exactly 1 MB is not fat
        let mut edge = good("edge", "personal");
        edge.size_bytes = FAT_SKILL_BYTES;
        assert!(analyze(&[edge], &[]).is_empty());
    }

    #[test]
    fn findings_are_ordered_crit_then_warn_then_info() {
        let mut fat = good("fat", "plugin");
        fat.size_bytes = FAT_SKILL_BYTES * 2;
        let skills = vec![
            fat,
            skill("weak", "personal", "Does stuff", "bad"),
            good("dup", "personal"),
            good("dup", "project"),
        ];
        let out = analyze(&skills, &[]);
        let sev: Vec<&str> = out.iter().map(|f| f.severity.as_str()).collect();
        assert_eq!(sev, vec!["crit", "warn", "info"]);
        assert_eq!(
            titles(&out),
            vec![
                "Skill name collision — dup",
                "Weak description — weak",
                "Fat skill — fat",
            ]
        );
    }

    /// The M0 pass deliberately ignores MCP state (`_mcp` is unused): a server
    /// in error state produces no finding today. Pinned here so that when MCP
    /// findings land, this test is updated on purpose rather than by accident.
    #[test]
    fn mcp_error_state_is_not_reported_in_m0() {
        let mcp = vec![
            mcp("broken", "error"),
            mcp("locked", "needs-auth"),
            mcp("ok", "connected"),
        ];
        assert!(analyze(&[good("verify", "personal")], &mcp).is_empty());
        assert!(analyze(&[], &mcp).is_empty());
    }

    #[test]
    fn truncate_is_char_based_and_trims() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("äöüß", 2), "äö…");
        assert_eq!(truncate("  padded  ", 10), "padded");
        assert_eq!(truncate("", 3), "");
    }
}
