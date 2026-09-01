use super::{EffectiveSkill, SkillScope};

pub(super) struct CatalogRender {
    pub(super) text: String,
    pub(super) included_count: usize,
}

impl CatalogRender {
    pub(super) fn all(text: String, count: usize) -> Self {
        Self {
            text,
            included_count: count,
        }
    }
}

const CODEX_INTRO: &str = "A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. `file` locators are on the host filesystem, `executor package` locators are owned by their execution environment, `orchestrator package` locators are opaque package identifiers, and `custom resource` locators use their provider's access mechanism.";
const CODEX_USAGE: &str = r#"- Discovery: The list above is the skills available in this session (name + description + source locator). `file` entries live on the host filesystem, `executor package` and `orchestrator package` entries are accessed directly through `skills.read`, and `custom resource` entries use their provider's access mechanism.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or its source can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, the main agent must read its `SKILL.md` completely before taking task actions. For a `file` entry, open the listed path. For an `executor package` or `orchestrator package`, pass the listed locator directly to `skills.read` as `package`; root aliases are resolved automatically. Omit `resource` to read `SKILL.md` directly without calling `skills.list`. If a read is paginated, follow `next_cursor` until EOF.
  2) When `SKILL.md` references another resource, use the same access mechanism. For executor and orchestrator skills, pass the complete package-contained resource identifier with the same package to `skills.read`; do not treat `skill://` identifiers as filesystem paths.
  3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the resources required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.
  4) For filesystem-backed skills, prefer running or patching provided scripts instead of retyping large code blocks. For executor and orchestrator skills, use `skills.read` and the available tools; do not invent a local path.
  5) Reuse provided assets or templates through the same source access mechanism instead of recreating them.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."#;

pub(super) fn render_codex_listing(skills: &[EffectiveSkill], budget: usize) -> CatalogRender {
    if skills.is_empty() {
        return CatalogRender::all(String::new(), 0);
    }
    let mut skills = skills.iter().collect::<Vec<_>>();
    skills.sort_by(|left, right| {
        codex_scope_order(left.scope)
            .cmp(&codex_scope_order(right.scope))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });
    let render = |skill: &EffectiveSkill, description: &str| {
        format!(
            "- {}: {} (file: {})",
            skill.name,
            description,
            skill.prompt_path.as_deref().unwrap_or_default()
        )
    };
    let full = skills
        .iter()
        .map(|skill| render(skill, &truncate_chars(&skill.description, 1_024)))
        .collect::<Vec<_>>()
        .join("\n");
    let (rows, included_count) = if full.len() <= budget {
        (full, skills.len())
    } else {
        let overhead = skills
            .iter()
            .map(|skill| render(skill, "").len() + 1)
            .sum::<usize>();
        let per_description = budget.saturating_sub(overhead) / skills.len().max(1);
        if per_description >= 20 {
            (
                skills
                    .iter()
                    .map(|skill| {
                        render(skill, &truncate_chars(&skill.description, per_description))
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                skills.len(),
            )
        } else {
            names_only(skills.into_iter().map(|skill| skill.name.as_str()), budget)
        }
    };
    CatalogRender::all(
        format!(
            "<skills_instructions>\n## Skills\n{CODEX_INTRO}\n### Available skills\n{rows}\n### How to use skills\n{CODEX_USAGE}\n</skills_instructions>"
        ),
        included_count,
    )
}

fn codex_scope_order(scope: SkillScope) -> u8 {
    match scope {
        SkillScope::System => 0,
        SkillScope::Repository | SkillScope::Local => 1,
        SkillScope::User => 2,
    }
}

pub(super) fn render_grok_listing(skills: &[EffectiveSkill], budget: usize) -> CatalogRender {
    if skills.is_empty() {
        return CatalogRender::all(String::new(), 0);
    }
    let header = "The following skills are available for use:\n\n";
    let render = |skill: &EffectiveSkill, combined_budget: usize| {
        let when_to_use = skill.when_to_use.as_deref().unwrap_or_default();
        let description_budget = if when_to_use.is_empty() {
            combined_budget
        } else {
            combined_budget * skill.description.len().max(1)
                / (skill.description.len().max(1) + when_to_use.len().max(1))
        };
        let when_budget = combined_budget.saturating_sub(description_budget);
        let description = truncate_chars(&skill.description, description_budget);
        let path = skill.prompt_path.as_deref().unwrap_or_default();
        if when_to_use.is_empty() {
            format!(
                "- {}: {}\n  Absolute path: {}",
                skill.name, description, path
            )
        } else {
            format!(
                "- {}: {}\n  Use when: {}\n  Absolute path: {}",
                skill.name,
                description,
                truncate_chars(when_to_use, when_budget),
                path
            )
        }
    };
    let full = skills
        .iter()
        .map(|skill| render(skill, 400))
        .collect::<Vec<_>>()
        .join("\n");
    if header.len() + full.len() <= budget {
        return CatalogRender::all(
            wrap_system_reminder(&format!("{header}{full}")),
            skills.len(),
        );
    }
    let per_entry = budget.saturating_sub(header.len()) / skills.len().max(1);
    if per_entry >= 20 {
        let shortened = skills
            .iter()
            .map(|skill| render(skill, per_entry.saturating_sub(2)))
            .collect::<Vec<_>>()
            .join("\n");
        return CatalogRender::all(
            wrap_system_reminder(&format!("{header}{shortened}")),
            skills.len(),
        );
    }
    let (rows, included_count) = names_only(
        skills.iter().map(|skill| skill.name.as_str()),
        budget.saturating_sub(header.len()),
    );
    CatalogRender::all(
        wrap_system_reminder(format!("{header}{rows}").trim_end()),
        included_count,
    )
}

fn wrap_system_reminder(content: &str) -> String {
    format!("<system-reminder>\n{content}\n</system-reminder>")
}

pub(super) fn render_claude_listing(entries: &[(String, String)], budget: usize) -> String {
    let render = |name: &str, description: &str| {
        if description.is_empty() {
            format!("- {name}")
        } else {
            format!("- {name}: {description}")
        }
    };
    let full = entries
        .iter()
        .map(|(name, description)| render(name, description))
        .collect::<Vec<_>>();
    if full.iter().map(String::len).sum::<usize>() + full.len().saturating_sub(1) <= budget {
        return full.join("\n");
    }
    let overhead = entries
        .iter()
        .map(|(name, _)| name.len() + 4)
        .sum::<usize>()
        + entries.len().saturating_sub(1);
    let description_budget = budget.saturating_sub(overhead) / entries.len().max(1);
    entries
        .iter()
        .map(|(name, description)| {
            if description_budget < 20 || description.is_empty() {
                format!("- {name}")
            } else {
                render(name, &truncate_chars(description, description_budget))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn names_only<'a>(names: impl Iterator<Item = &'a str>, budget: usize) -> (String, usize) {
    let mut rows = Vec::new();
    let mut used = 0usize;
    for name in names {
        let row = format!("- {name}");
        let next = row.len() + usize::from(!rows.is_empty());
        if used + next > budget {
            break;
        }
        used += next;
        rows.push(row);
    }
    let count = rows.len();
    (rows.join("\n"), count)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut result = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::effective_skills::{SkillSource, SkillVisibility};

    #[test]
    fn claude_falls_back_to_names_only_inside_budget() {
        let entries = vec![
            ("one".to_string(), "x".repeat(200)),
            ("two".to_string(), "y".repeat(200)),
        ];
        let rendered = render_claude_listing(&entries, 20);
        assert_eq!(rendered, "- one\n- two");
        assert!(rendered.len() <= 20);
    }

    #[test]
    fn codex_reports_only_entries_that_fit_the_model_visible_catalog() {
        let skills = ["first-long-name", "second-long-name"]
            .into_iter()
            .map(|name| EffectiveSkill {
                id: name.to_string(),
                name: name.to_string(),
                description: "description".repeat(20),
                when_to_use: None,
                path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
                root_path: Some(PathBuf::from("/skills")),
                prompt_path: Some(format!("/skills/{name}/SKILL.md")),
                scope: SkillScope::User,
                visibility: SkillVisibility::Automatic,
                source: SkillSource::Filesystem,
            })
            .collect::<Vec<_>>();
        let rendered = render_codex_listing(&skills, 10);
        assert_eq!(rendered.included_count, 0);
        assert!(!rendered.text.contains("- first-long-name"));
    }
}
