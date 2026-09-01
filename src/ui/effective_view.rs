use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    ProjectSkill, ProjectSkillInstallation,
    effective_skills::{
        AgentContextEstimate, AgentKind, SkillSource, SkillVisibility, group_effective_skills,
    },
    project,
};

#[derive(Clone)]
pub(super) struct EffectivePluginGroup {
    pub key: String,
    pub agent: AgentKind,
    pub display_name: String,
    pub skills: Vec<String>,
}

pub(super) fn plugin_groups(
    estimates: &[AgentContextEstimate],
    selected_agent: Option<AgentKind>,
) -> Vec<EffectivePluginGroup> {
    let mut groups = BTreeMap::<String, EffectivePluginGroup>::new();
    for estimate in estimates
        .iter()
        .filter(|estimate| selected_agent.is_none_or(|agent| agent == estimate.agent))
    {
        for skill in estimate.plugin_skills() {
            let Some(id) = skill.source.plugin_id() else {
                continue;
            };
            let display_name = skill.source.plugin_display_name().unwrap_or(id);
            let key = format!("{}:{id}", estimate.agent.id());
            let group = groups
                .entry(key.clone())
                .or_insert_with(|| EffectivePluginGroup {
                    key,
                    agent: estimate.agent,
                    display_name: display_name.to_string(),
                    skills: Vec::new(),
                });
            if !group.skills.contains(&skill.name) {
                group.skills.push(skill.name.clone());
            }
        }
    }
    for group in groups.values_mut() {
        group.skills.sort();
    }
    groups.into_values().collect()
}

#[derive(Clone)]
pub(super) struct EffectiveSkillRow {
    pub name: String,
    pub description: String,
    pub locations: Vec<PathBuf>,
    pub built_in: bool,
    pub agents: Vec<AgentKind>,
    pub manual_only: bool,
    pub direct_installations: Vec<ProjectSkillInstallation>,
}

pub(super) fn same_file(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

pub(super) fn skill_rows(
    estimates: &[AgentContextEstimate],
    project_skills: &[ProjectSkill],
    selected_agent: Option<AgentKind>,
) -> Vec<EffectiveSkillRow> {
    let groups = group_effective_skills(
        estimates
            .iter()
            .filter(|estimate| selected_agent.is_none_or(|agent| agent == estimate.agent))
            .flat_map(|estimate| {
                estimate
                    .skills
                    .iter()
                    .filter(|skill| !skill.is_plugin())
                    .map(|skill| (estimate.agent, skill))
            }),
    );
    let mut rows = Vec::with_capacity(groups.len());
    for group in groups {
        let Some((_, first)) = group.entries().first() else {
            continue;
        };
        let mut row = EffectiveSkillRow {
            name: first.name.clone(),
            description: first.description.clone(),
            locations: Vec::new(),
            built_in: false,
            agents: Vec::new(),
            manual_only: false,
            direct_installations: Vec::new(),
        };
        for (agent, skill) in group.entries() {
            if row.description.is_empty() && !skill.description.is_empty() {
                row.description = skill.description.clone();
            }
            if let Some(root) = &skill.root_path
                && !row.locations.iter().any(|location| location == root)
            {
                row.locations.push(root.clone());
            }
            row.built_in |= matches!(skill.source, SkillSource::Builtin);
            if !row.agents.contains(agent) {
                row.agents.push(*agent);
            }
            row.manual_only |= skill.visibility == SkillVisibility::ManualOnly;

            for installation in project_skills
                .iter()
                .filter(|installed| installed.name == skill.name)
                .flat_map(|installed| &installed.installations)
            {
                if same_file(&installation.path.join("SKILL.md"), &skill.path)
                    && !row.direct_installations.iter().any(|existing| {
                        project::installation_key(&existing.path)
                            == project::installation_key(&installation.path)
                    })
                {
                    row.direct_installations.push(installation.clone());
                }
            }
        }
        rows.push(row);
    }
    let agent_order = |agent: &AgentKind| {
        crate::agents::AGENT_ICON_ORDER
            .iter()
            .position(|item| item.id == agent.id())
            .unwrap_or(usize::MAX)
    };
    for row in &mut rows {
        row.locations.sort_by_key(|path| super::display_path(path));
        row.agents.sort_by_key(agent_order);
    }
    rows
}
