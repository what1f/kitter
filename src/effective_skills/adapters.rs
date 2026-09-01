//! The production seam for effective-Skill inspection.
//!
//! Each Agent adapter owns the static scan and metadata profile used before
//! its entry rules render the initial model catalog. Runtime loading is not
//! part of this module.

use std::collections::HashMap;

use super::scanner::ScanProfile;
use super::{
    AgentContextEstimate, AmpPolicy, AntigravityPolicy, ClaudeCodeAdapter, CodexAdapter,
    CopilotPolicy, CursorPolicy, DiscoveryContext, DroidPolicy, EffectiveSkill, GrokAdapter,
    HermesPolicy, MetadataProfile, OpenClawPolicy, OpenCodePolicy, PiAdapter, SkillScope, discover,
    estimate_skills, estimate_with_policy, estimate_with_profile, normalize_grok_name,
};

trait AgentAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate;
}

macro_rules! legacy_adapter {
    ($adapter:ident, $policy:ident) => {
        struct $adapter;
        impl AgentAdapter for $adapter {
            fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
                estimate_with_policy(&$policy, context)
            }
        }
    };
}

impl AgentAdapter for CodexAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::Recursive {
                max_depth: 6,
                max_directories: 2_000,
                max_entries: 20_000,
            },
            MetadataProfile::StrictFrontmatter,
        )
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::DirectChildren,
            MetadataProfile::BodyFallback,
        )
    }
}

impl AgentAdapter for PiAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::PiIgnored,
            MetadataProfile::StrictFrontmatter,
        )
    }
}

impl AgentAdapter for GrokAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        let skills = discover(
            self,
            context,
            ScanProfile::Recursive {
                max_depth: 5,
                max_directories: usize::MAX,
                max_entries: usize::MAX,
            },
            MetadataProfile::BodyFallback,
        );
        estimate_skills(self, resolve_grok_collisions(skills))
    }
}

fn resolve_grok_collisions(skills: Vec<EffectiveSkill>) -> Vec<EffectiveSkill> {
    let mut resolved: Vec<EffectiveSkill> = Vec::with_capacity(skills.len());
    let mut names: HashMap<String, (SkillScope, usize)> = HashMap::new();
    for mut skill in skills {
        let Some(&(winner_scope, winner_index)) = names.get(&skill.id) else {
            names.insert(skill.id.clone(), (skill.scope, resolved.len()));
            resolved.push(skill);
            continue;
        };
        if winner_scope != skill.scope {
            continue;
        }
        let directory_name = grok_path_identity(&skill);
        if directory_name != skill.id && !names.contains_key(&directory_name) {
            skill.id = directory_name.clone();
            skill.name = directory_name.clone();
            names.insert(directory_name, (skill.scope, resolved.len()));
            resolved.push(skill);
            continue;
        }
        if directory_name == skill.id {
            let winner_directory = grok_path_identity(&resolved[winner_index]);
            if winner_directory != skill.id && !names.contains_key(&winner_directory) {
                let contested = skill.id.clone();
                resolved[winner_index].id = winner_directory.clone();
                resolved[winner_index].name = winner_directory.clone();
                names.remove(&contested);
                names.insert(winner_directory, (winner_scope, winner_index));
                names.insert(contested, (skill.scope, resolved.len()));
                resolved.push(skill);
            }
        }
    }
    resolved
}

fn grok_path_identity(skill: &EffectiveSkill) -> String {
    let raw = if skill
        .path
        .file_name()
        .is_some_and(|name| name == "SKILL.md")
    {
        skill
            .path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
    } else {
        skill.path.file_stem().and_then(|name| name.to_str())
    };
    raw.map(normalize_grok_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| skill.id.clone())
}

legacy_adapter!(CursorAdapter, CursorPolicy);
legacy_adapter!(OpenCodeAdapter, OpenCodePolicy);
legacy_adapter!(CopilotAdapter, CopilotPolicy);
legacy_adapter!(AntigravityAdapter, AntigravityPolicy);
legacy_adapter!(AmpAdapter, AmpPolicy);
legacy_adapter!(DroidAdapter, DroidPolicy);
legacy_adapter!(OpenClawAdapter, OpenClawPolicy);
legacy_adapter!(HermesAdapter, HermesPolicy);

pub(super) fn inspect_project(
    context: &DiscoveryContext,
    include_global_only: bool,
) -> Vec<AgentContextEstimate> {
    let adapters: [&dyn AgentAdapter; 10] = [
        &CodexAdapter,
        &ClaudeCodeAdapter,
        &CursorAdapter,
        &OpenCodeAdapter,
        &CopilotAdapter,
        &AntigravityAdapter,
        &AmpAdapter,
        &DroidAdapter,
        &PiAdapter,
        &GrokAdapter,
    ];
    let mut estimates = adapters
        .into_iter()
        .map(|adapter| adapter.inspect(context))
        .collect::<Vec<_>>();
    if include_global_only {
        estimates.extend([
            OpenClawAdapter.inspect(context),
            HermesAdapter.inspect(context),
        ]);
    }
    estimates
}
