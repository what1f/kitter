//! Agent-specific discovery and context-cost estimation for effective skills.
//!
//! The public interface returns an initial-context snapshot. Agent adapters own
//! static discovery profiles; scanner and catalog modules are internal primitives.

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

mod adapters;
mod catalog;
mod scanner;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Codex,
    ClaudeCode,
    Cursor,
    OpenCode,
    Copilot,
    Antigravity,
    Amp,
    Droid,
    Pi,
    Grok,
    OpenClaw,
    Hermes,
}

impl AgentKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
            Self::Antigravity => "antigravity",
            Self::Amp => "amp",
            Self::Droid => "droid",
            Self::Pi => "pi",
            Self::Grok => "grok",
            Self::OpenClaw => "openclaw",
            Self::Hermes => "hermes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::OpenCode => "OpenCode",
            Self::Copilot => "GitHub Copilot",
            Self::Antigravity => "Antigravity",
            Self::Amp => "Amp",
            Self::Droid => "Droid",
            Self::Pi => "Pi",
            Self::Grok => "Grok",
            Self::OpenClaw => "OpenClaw",
            Self::Hermes => "Hermes",
        }
    }

    pub fn is_global_only(self) -> bool {
        matches!(self, Self::OpenClaw | Self::Hermes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillVisibility {
    Automatic,
    NameOnly,
    ManualOnly,
    Conditional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    Local,
    Repository,
    User,
    System,
}

/// Provenance for an effective Skill or a discovery root.
///
/// This is intentionally separate from [`SkillScope`]. A plugin can be
/// installed in a user or system scope while still needing its own UI group.
///
/// Plugin identity is the stable provider identity (for example
/// `deploy@official`), while `display_name` is the human-facing name read
/// from the provider manifest or derived from that identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillSource {
    Filesystem,
    Builtin,
    Plugin { id: String, display_name: String },
}

impl SkillSource {
    pub fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin { .. })
    }

    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            Self::Plugin { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn plugin_display_name(&self) -> Option<&str> {
        match self {
            Self::Plugin { display_name, .. } => Some(display_name),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectiveSkill {
    /// Provider registration ID. Most providers use the frontmatter name;
    /// OpenCode V2 derives it from the source path.
    pub id: String,
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub path: PathBuf,
    /// Discovery root that made this Skill effective. The Projects UI uses
    /// this exact value for provenance instead of trying to reconstruct a
    /// provider-specific user/project directory from the Skill file path.
    pub root_path: Option<PathBuf>,
    /// The path string, if this provider actually places one in its model
    /// catalog. This is intentionally not inferred from [`Self::path`].
    pub prompt_path: Option<String>,
    pub scope: SkillScope,
    pub visibility: SkillVisibility,
    pub source: SkillSource,
}

impl EffectiveSkill {
    pub fn is_plugin(&self) -> bool {
        self.source.is_plugin()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentContextEstimate {
    pub agent: AgentKind,
    pub discovered_count: usize,
    pub model_visible_count: usize,
    pub manual_only_count: usize,
    pub name_only_count: usize,
    pub conditional_count: usize,
    pub estimated_tokens: usize,
    pub skills: Vec<EffectiveSkill>,
}

impl AgentContextEstimate {
    /// Plugin contributions are kept in the same effective set as filesystem
    /// Skills, but this view lets the UI render a plugin-specific tab without
    /// reimplementing source classification.
    pub fn plugin_skills(&self) -> impl Iterator<Item = &EffectiveSkill> {
        self.skills.iter().filter(|skill| skill.is_plugin())
    }
}

pub struct EffectiveSkillGroup<'a> {
    entries: Vec<(AgentKind, &'a EffectiveSkill)>,
}

impl<'a> EffectiveSkillGroup<'a> {
    pub fn name(&self) -> &str {
        &self.entries[0].1.name
    }

    pub fn entries(&self) -> &[(AgentKind, &'a EffectiveSkill)] {
        &self.entries
    }
}

/// Groups the same logical Skill across Agent snapshots.
///
/// Agents can expose different names for one filesystem entry (for example,
/// Claude's `build_from_zero` and Grok's normalized `build-from-zero`). A
/// matching name or canonical filesystem path joins entries into one group.
pub fn group_effective_skills<'a>(
    entries: impl IntoIterator<Item = (AgentKind, &'a EffectiveSkill)>,
) -> Vec<EffectiveSkillGroup<'a>> {
    struct Candidate<'a> {
        agent: AgentKind,
        skill: &'a EffectiveSkill,
        filesystem_path: Option<PathBuf>,
    }

    let mut groups = Vec::<Vec<Candidate<'a>>>::new();
    for (agent, skill) in entries {
        let filesystem_path = matches!(skill.source, SkillSource::Filesystem).then(|| {
            skill
                .path
                .canonicalize()
                .unwrap_or_else(|_| skill.path.clone())
        });
        let candidate = Candidate {
            agent,
            skill,
            filesystem_path,
        };
        let matching = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                group
                    .iter()
                    .any(|entry| {
                        entry.skill.name == candidate.skill.name
                            || entry
                                .filesystem_path
                                .as_ref()
                                .zip(candidate.filesystem_path.as_ref())
                                .is_some_and(|(left, right)| left == right)
                    })
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            groups.push(vec![candidate]);
            continue;
        }

        let mut merged = Vec::new();
        for index in matching.into_iter().rev() {
            let mut group = groups.remove(index);
            group.append(&mut merged);
            merged = group;
        }
        merged.push(candidate);
        groups.push(merged);
    }

    let mut result = groups
        .into_iter()
        .map(|group| EffectiveSkillGroup {
            entries: group
                .into_iter()
                .map(|entry| (entry.agent, entry.skill))
                .collect(),
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.name().cmp(right.name()));
    result
}

#[derive(Clone, Debug)]
struct DiscoveryContext {
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub repository_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct SkillRoot {
    pub path: PathBuf,
    pub scope: SkillScope,
    pub include_root_markdown: bool,
    pub flat_markdown_only: bool,
    pub direct_children_only: bool,
    pub follow_directory_symlinks: bool,
    pub exact_skill_file: Option<PathBuf>,
    pub source: SkillSource,
}

impl SkillRoot {
    fn new(path: PathBuf, scope: SkillScope) -> Self {
        Self {
            path,
            scope,
            include_root_markdown: false,
            flat_markdown_only: false,
            direct_children_only: false,
            follow_directory_symlinks: true,
            exact_skill_file: None,
            source: SkillSource::Filesystem,
        }
    }

    fn plugin(
        path: PathBuf,
        scope: SkillScope,
        id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let mut root = Self::new(path, scope);
        root.source = SkillSource::Plugin {
            id: id.into(),
            display_name: display_name.into(),
        };
        root
    }

    fn with_source(mut self, source: SkillSource) -> Self {
        self.source = source;
        self
    }

    fn with_root_markdown(mut self) -> Self {
        self.include_root_markdown = true;
        self
    }

    fn flat_markdown(mut self) -> Self {
        self.include_root_markdown = true;
        self.flat_markdown_only = true;
        self
    }

    fn direct_children(mut self) -> Self {
        self.direct_children_only = true;
        self
    }

    fn without_directory_symlinks(mut self) -> Self {
        self.follow_directory_symlinks = false;
        self
    }

    fn exact(skill_file: PathBuf, scope: SkillScope) -> Self {
        Self {
            path: skill_file
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            scope,
            include_root_markdown: true,
            flat_markdown_only: false,
            direct_children_only: false,
            follow_directory_symlinks: true,
            exact_skill_file: Some(skill_file),
            source: SkillSource::Filesystem,
        }
    }
}

#[derive(Clone, Debug)]
struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub has_explicit_description: bool,
    pub when_to_use: Option<String>,
    pub source_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameCollision {
    KeepAll,
    FirstWins,
    LastWins,
}

/// Internal entry rules composed by the Agent adapters.
trait AgentSkillPolicy {
    fn agent(&self) -> AgentKind;
    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot>;
    fn builtins(&self, _context: &DiscoveryContext) -> Vec<EffectiveSkill> {
        Vec::new()
    }
    fn is_enabled(
        &self,
        _skill_dir: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        true
    }
    fn effective_name(&self, _skill_dir: &Path, metadata: &SkillMetadata) -> String {
        metadata.name.clone()
    }
    fn effective_name_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
    ) -> String {
        let _ = root;
        self.effective_name(skill_dir, metadata)
    }
    fn effective_id_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        skill_file: &Path,
        metadata: &SkillMetadata,
    ) -> String {
        let _ = skill_file;
        self.effective_name_for_root(root, skill_dir, metadata)
    }
    fn prompt_path_for_entry(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        skill_file: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> Option<String> {
        let _ = (root, skill_dir, skill_file, metadata, context);
        None
    }
    fn is_enabled_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        let _ = root;
        self.is_enabled(skill_dir, metadata, context)
    }
    fn is_enabled_for_entry(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        skill_file: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        let _ = skill_file;
        self.is_enabled_for_root(root, skill_dir, metadata, context)
    }
    fn name_collision(&self) -> NameCollision;
    fn visibility(
        &self,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> SkillVisibility;
    fn visibility_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> SkillVisibility {
        let _ = root;
        self.visibility(skill_dir, metadata, context)
    }
    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String;
    fn render_initial_catalog(&self, skills: &[EffectiveSkill]) -> catalog::CatalogRender {
        catalog::CatalogRender::all(self.render_visible_metadata(skills), skills.len())
    }
    fn estimate_tokens(&self, rendered: &str) -> usize;
}

struct CodexAdapter;
struct ClaudeCodeAdapter;
struct CursorPolicy;
struct OpenCodePolicy;
struct CopilotPolicy;
struct AntigravityPolicy;
struct AmpPolicy;
struct DroidPolicy;
struct PiAdapter;
struct GrokAdapter;
struct OpenClawPolicy;
struct HermesPolicy;

#[cfg(test)]
#[allow(non_upper_case_globals)]
const CodexPolicy: CodexAdapter = CodexAdapter;
#[cfg(test)]
#[allow(non_upper_case_globals)]
const ClaudeCodePolicy: ClaudeCodeAdapter = ClaudeCodeAdapter;
#[cfg(test)]
#[allow(non_upper_case_globals)]
const PiPolicy: PiAdapter = PiAdapter;
#[cfg(test)]
#[allow(non_upper_case_globals)]
const GrokPolicy: GrokAdapter = GrokAdapter;

pub fn estimate_project(project: &Path) -> Vec<AgentContextEstimate> {
    let home = dirs::home_dir().unwrap_or_else(|| project.to_path_buf());
    estimate_project_with_home(project, &home)
}

pub fn estimate_project_with_home(project: &Path, home: &Path) -> Vec<AgentContextEstimate> {
    let context = DiscoveryContext {
        cwd: project.to_path_buf(),
        home: home.to_path_buf(),
        repository_root: find_repository_root(project),
    };
    adapters::inspect_project(&context, project == home)
}

#[derive(Clone, Copy)]
enum MetadataProfile {
    StrictFrontmatter,
    BodyFallback,
}

fn estimate_with_policy(
    policy: &dyn AgentSkillPolicy,
    context: &DiscoveryContext,
) -> AgentContextEstimate {
    estimate_with_profile(
        policy,
        context,
        scanner::ScanProfile::Recursive {
            max_depth: usize::MAX,
            max_directories: usize::MAX,
            max_entries: usize::MAX,
        },
        MetadataProfile::StrictFrontmatter,
    )
}

fn estimate_with_profile(
    policy: &dyn AgentSkillPolicy,
    context: &DiscoveryContext,
    scan_profile: scanner::ScanProfile,
    metadata_profile: MetadataProfile,
) -> AgentContextEstimate {
    let skills = discover(policy, context, scan_profile, metadata_profile);
    estimate_skills(policy, skills)
}

fn estimate_skills(
    policy: &dyn AgentSkillPolicy,
    skills: Vec<EffectiveSkill>,
) -> AgentContextEstimate {
    let visible = skills
        .iter()
        .filter(|skill| {
            matches!(
                skill.visibility,
                SkillVisibility::Automatic | SkillVisibility::NameOnly
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let rendered = policy.render_initial_catalog(&visible);
    AgentContextEstimate {
        agent: policy.agent(),
        discovered_count: skills.len(),
        model_visible_count: rendered.included_count,
        manual_only_count: skills
            .iter()
            .filter(|s| s.visibility == SkillVisibility::ManualOnly)
            .count(),
        name_only_count: skills
            .iter()
            .filter(|s| s.visibility == SkillVisibility::NameOnly)
            .count(),
        conditional_count: skills
            .iter()
            .filter(|s| s.visibility == SkillVisibility::Conditional)
            .count(),
        estimated_tokens: policy.estimate_tokens(&rendered.text),
        skills,
    }
}

pub fn is_manual_skill(skill_dir: &Path) -> bool {
    let Some(metadata) = read_metadata(&skill_dir.join("SKILL.md")) else {
        return false;
    };
    frontmatter_visibility(&metadata) == SkillVisibility::ManualOnly || codex_manual_only(skill_dir)
}

fn discover(
    policy: &dyn AgentSkillPolicy,
    context: &DiscoveryContext,
    scan_profile: scanner::ScanProfile,
    metadata_profile: MetadataProfile,
) -> Vec<EffectiveSkill> {
    let mut result = policy.builtins(context);
    let mut ids = result
        .iter()
        .map(|skill| skill.id.clone())
        .collect::<HashSet<_>>();
    let mut canonical_paths = HashSet::new();
    for root in policy.roots(context) {
        for skill_file in scanner::scan(&root, scan_profile) {
            let skill_dir = skill_file.parent().unwrap_or(&root.path).to_path_buf();
            // Directory-based Skills deduplicate by their directory, while
            // Pi roots and explicit paths may contain several independent
            // root-level Markdown files.  Keying those files by the parent
            // directory would silently drop every file after the first one.
            let canonical_target = if root.exact_skill_file.is_some()
                || (root.include_root_markdown && skill_dir == root.path)
            {
                &skill_file
            } else {
                &skill_dir
            };
            let canonical = fs::canonicalize(canonical_target)
                .unwrap_or_else(|_| canonical_target.to_path_buf());
            if !canonical_paths.insert(canonical) {
                continue;
            }
            let Some(metadata) = read_metadata_with_profile(&skill_file, metadata_profile) else {
                continue;
            };
            if !policy.is_enabled_for_entry(&root, &skill_dir, &skill_file, &metadata, context) {
                continue;
            }
            let name = policy.effective_name_for_root(&root, &skill_dir, &metadata);
            let id = policy.effective_id_for_root(&root, &skill_dir, &skill_file, &metadata);
            match policy.name_collision() {
                NameCollision::KeepAll => {}
                NameCollision::FirstWins if !ids.insert(id.clone()) => continue,
                NameCollision::FirstWins => {}
                NameCollision::LastWins => {
                    if ids.insert(id.clone()) {
                        // First occurrence; nothing to replace.
                    } else if let Some(index) = result
                        .iter()
                        .position(|skill: &EffectiveSkill| skill.id == id)
                    {
                        result.remove(index);
                    }
                }
            }
            let prompt_path =
                policy.prompt_path_for_entry(&root, &skill_dir, &skill_file, &metadata, context);
            result.push(EffectiveSkill {
                id,
                visibility: policy.visibility_for_root(&root, &skill_dir, &metadata, context),
                name,
                description: metadata.description,
                when_to_use: metadata.when_to_use,
                path: skill_file,
                root_path: Some(root.path.clone()),
                prompt_path,
                scope: root.scope,
                source: root.source.clone(),
            });
        }
    }
    result
}

impl AgentSkillPolicy for CodexAdapter {
    fn agent(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let mut roots = Vec::new();
        if !is_global_context(context) {
            let project_config_root = context.repository_root.as_deref().unwrap_or(&context.cwd);
            roots.push(SkillRoot::new(
                project_config_root.join(".codex/skills"),
                SkillScope::Repository,
            ));
            if project_config_root != context.cwd {
                roots.push(SkillRoot::new(
                    context.cwd.join(".codex/skills"),
                    SkillScope::Local,
                ));
            }
            for dir in cwd_to_boundary(&context.cwd, context.repository_root.as_deref()) {
                roots.push(SkillRoot::new(
                    dir.join(".agents/skills"),
                    SkillScope::Repository,
                ));
            }
        }
        let codex_home = codex_home(context);
        roots.push(SkillRoot::new(codex_home.join("skills"), SkillScope::User));
        roots.push(SkillRoot::new(
            context.home.join(".agents/skills"),
            SkillScope::User,
        ));
        roots.push(
            SkillRoot::new(codex_home.join("skills/.system"), SkillScope::System)
                .without_directory_symlinks(),
        );
        #[cfg(unix)]
        roots.push(SkillRoot::new(
            PathBuf::from("/etc/codex/skills"),
            SkillScope::System,
        ));
        roots.extend(codex_plugin_roots(&codex_home));
        roots
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        if !metadata.has_explicit_description {
            return false;
        }
        let disabled = codex_disabled_skills(&codex_home(context), &context.home);
        let source_file = fs::canonicalize(&metadata.source_path)
            .unwrap_or_else(|_| metadata.source_path.clone());
        let source_dir = metadata
            .source_path
            .parent()
            .map(|path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
        !disabled.contains(&source_file) && !source_dir.is_some_and(|path| disabled.contains(&path))
    }

    fn prompt_path_for_entry(
        &self,
        _root: &SkillRoot,
        _skill_dir: &Path,
        skill_file: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> Option<String> {
        Some(skill_file.display().to_string())
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::KeepAll
    }

    fn visibility(
        &self,
        skill_dir: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        if codex_manual_only(skill_dir) {
            SkillVisibility::ManualOnly
        } else {
            SkillVisibility::Automatic
        }
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        catalog::render_codex_listing(skills, 8_000).text
    }

    fn render_initial_catalog(&self, skills: &[EffectiveSkill]) -> catalog::CatalogRender {
        catalog::render_codex_listing(skills, 8_000)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for ClaudeCodeAdapter {
    fn agent(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let mut roots = Vec::new();
        #[cfg(target_os = "macos")]
        {
            let managed = PathBuf::from("/Library/Application Support/ClaudeCode/.claude");
            roots.push(SkillRoot::new(managed.join("skills"), SkillScope::System));
            roots
                .push(SkillRoot::new(managed.join("commands"), SkillScope::System).flat_markdown());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        roots.push(SkillRoot::new(
            PathBuf::from("/etc/claude-code/.claude/skills"),
            SkillScope::System,
        ));
        let user_config = claude_config_dir(context);
        roots.push(SkillRoot::new(user_config.join("skills"), SkillScope::User));
        roots.push(SkillRoot::new(user_config.join("commands"), SkillScope::User).flat_markdown());
        if !is_global_context(context) {
            for dir in cwd_to_boundary(&context.cwd, context.repository_root.as_deref()) {
                roots.push(SkillRoot::new(
                    dir.join(".claude/skills"),
                    SkillScope::Repository,
                ));
                roots.push(
                    SkillRoot::new(dir.join(".claude/commands"), SkillScope::Repository)
                        .flat_markdown(),
                );
            }
        }
        roots.extend(claude_plugin_roots(context));
        roots
    }

    fn effective_name(&self, skill_dir: &Path, metadata: &SkillMetadata) -> String {
        let local_name = claude_local_name(skill_dir, metadata);
        claude_plugin_name(skill_dir)
            .map(|plugin| format!("{plugin}:{local_name}"))
            .unwrap_or(local_name)
    }

    fn effective_name_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
    ) -> String {
        let local_name = claude_local_name(skill_dir, metadata);
        root.source
            .plugin_display_name()
            .map(|plugin| format!("{plugin}:{local_name}"))
            .unwrap_or_else(|| self.effective_name(skill_dir, metadata))
    }

    fn is_enabled(
        &self,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        if claude_plugin_name(skill_dir).is_some() {
            return metadata.has_explicit_description || metadata.when_to_use.is_some();
        }
        claude_skill_override(context, &self.effective_name(skill_dir, metadata)).as_deref()
            != Some("off")
    }

    fn is_enabled_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        if root.source.is_plugin() {
            // Claude has no separate per-Skill plugin enable switch.  The
            // plugin registry/settings already established enablement before
            // this root was created; frontmatter only controls invocation.
            metadata.has_explicit_description || metadata.when_to_use.is_some()
        } else {
            self.is_enabled(skill_dir, metadata, context)
        }
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::KeepAll
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> SkillVisibility {
        if claude_plugin_name(_skill_dir).is_some() {
            return frontmatter_visibility(metadata);
        }
        match claude_skill_override(context, &self.effective_name(_skill_dir, metadata)).as_deref()
        {
            Some("name-only") => SkillVisibility::NameOnly,
            Some("user-invocable-only") => SkillVisibility::ManualOnly,
            Some("on") => SkillVisibility::Automatic,
            _ => frontmatter_visibility(metadata),
        }
    }

    fn visibility_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> SkillVisibility {
        if root.source.is_plugin() {
            frontmatter_visibility(metadata)
        } else {
            self.visibility(skill_dir, metadata, context)
        }
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        let entries = skills
            .iter()
            .map(|skill| -> (String, String) {
                if skill.visibility == SkillVisibility::NameOnly {
                    return (skill.name.clone(), String::new());
                }
                let description = match &skill.when_to_use {
                    Some(when_to_use) if !when_to_use.is_empty() => {
                        format!("{} - {when_to_use}", skill.description)
                    }
                    _ => skill.description.clone(),
                };
                (skill.name.clone(), truncate_chars(&description, 250))
            })
            .collect::<Vec<_>>();
        format!(
            "<system-reminder>\n{}\n</system-reminder>",
            catalog::render_claude_listing(&entries, 8_000)
        )
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for CursorPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Cursor
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        compatible_filesystem_roots(
            context,
            &[
                ".cursor/skills",
                ".agents/skills",
                ".claude/skills",
                ".codex/skills",
            ],
            &[
                ".cursor/skills",
                ".agents/skills",
                ".claude/skills",
                ".codex/skills",
            ],
            None,
        )
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        compatible_skill_is_valid(metadata)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        frontmatter_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        render_name_description_catalog(skills)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for OpenCodePolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::OpenCode
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let config_home = opencode_config_home(context);
        let mut roots = Vec::new();
        let ancestor_dirs = if is_global_context(context) {
            Vec::new()
        } else {
            cwd_to_boundary(&context.cwd, context.repository_root.as_deref())
        };
        let disable_external = env_truthy("OPENCODE_DISABLE_EXTERNAL_SKILLS");
        let disable_claude = env_truthy("OPENCODE_DISABLE_CLAUDE_CODE")
            || env_truthy("OPENCODE_DISABLE_CLAUDE_CODE_SKILLS");
        if !disable_external {
            if !disable_claude {
                roots.push(
                    SkillRoot::new(context.home.join(".claude/skills"), SkillScope::User)
                        .with_root_markdown(),
                );
                for dir in ancestor_dirs.iter().rev() {
                    roots.push(
                        SkillRoot::new(dir.join(".claude/skills"), SkillScope::Repository)
                            .with_root_markdown(),
                    );
                }
            }
            roots.push(
                SkillRoot::new(context.home.join(".agents/skills"), SkillScope::User)
                    .with_root_markdown(),
            );
            for dir in ancestor_dirs.iter().rev() {
                roots.push(
                    SkillRoot::new(dir.join(".agents/skills"), SkillScope::Repository)
                        .with_root_markdown(),
                );
            }
        }
        for relative in ["skills", "skill"] {
            roots.push(
                SkillRoot::new(config_home.join(relative), SkillScope::User).with_root_markdown(),
            );
        }
        for dir in ancestor_dirs.iter().rev() {
            for relative in ["skills", "skill"] {
                roots.push(
                    SkillRoot::new(dir.join(".opencode").join(relative), SkillScope::Repository)
                        .with_root_markdown(),
                );
            }
        }
        if let Some(custom) = env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from) {
            for relative in ["skill", "skills"] {
                roots.push(
                    SkillRoot::new(custom.join(relative), SkillScope::User).with_root_markdown(),
                );
            }
        }
        #[cfg(target_os = "macos")]
        for relative in ["skill", "skills"] {
            roots.push(
                SkillRoot::new(
                    PathBuf::from("/Library/Application Support/opencode").join(relative),
                    SkillScope::System,
                )
                .with_root_markdown(),
            );
        }
        roots.extend(
            opencode_config(context)
                .skill_paths
                .into_iter()
                .map(|(path, scope)| SkillRoot::new(path, scope).with_root_markdown()),
        );
        roots
    }

    fn builtins(&self, context: &DiscoveryContext) -> Vec<EffectiveSkill> {
        if opencode_skill_permission(context, "customize-opencode") == "deny" {
            return Vec::new();
        }
        vec![EffectiveSkill {
            id: "customize-opencode".to_string(),
            name: "customize-opencode".to_string(),
            description: "Use ONLY when the user is editing or creating opencode's own configuration: opencode.json, opencode.jsonc, files under .opencode/, or files under ~/.config/opencode/. Also use when creating or fixing opencode agents, subagents, skills, plugins, MCP servers, or permission rules. Do not use for the user's own application code, or for any project that is not configuring opencode itself.".to_string(),
            when_to_use: None,
            path: PathBuf::from("<built-in>"),
            root_path: None,
            prompt_path: None,
            scope: SkillScope::System,
            visibility: SkillVisibility::Automatic,
            source: SkillSource::Builtin,
        }]
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        _metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        opencode_skill_permission(context, "*") != "deny"
    }

    fn is_enabled_for_entry(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        skill_file: &Path,
        _metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        opencode_skill_permission(context, &path_derived_skill_id(root, skill_dir, skill_file))
            != "deny"
    }

    fn effective_id_for_root(
        &self,
        root: &SkillRoot,
        skill_dir: &Path,
        skill_file: &Path,
        _metadata: &SkillMetadata,
    ) -> String {
        path_derived_skill_id(root, skill_dir, skill_file)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::LastWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        opencode_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        let body = skills
            .iter()
            .map(|skill| {
                format!(
                    "  <skill>\n    <id>{}</id>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>",
                    xml_escape(&skill.id),
                    xml_escape(&skill.name),
                    xml_escape(&skill.description)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Skills provide specialized instructions and workflows for specific tasks.\nUse the skill tool to load a skill when a task matches its description.\n<available_skills>\n{body}\n</available_skills>"
        )
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for CopilotPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Copilot
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        compatible_filesystem_roots(
            context,
            &[".github/skills", ".agents/skills", ".claude/skills"],
            &[".copilot/skills", ".agents/skills", ".claude/skills"],
            Some("COPILOT_SKILLS_DIRS"),
        )
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        compatible_skill_is_valid(metadata)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        frontmatter_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        render_name_description_catalog(skills)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for AntigravityPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Antigravity
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        compatible_filesystem_roots(
            context,
            &[".agents/skills", ".agent/skills"],
            &[".gemini/config/skills", ".agents/skills", ".agent/skills"],
            None,
        )
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        compatible_skill_is_valid(metadata)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        // Antigravity documents model-routed Skills and separate user-run
        // Workflows, but no Skill frontmatter switch for manual-only use.
        SkillVisibility::Automatic
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        render_name_description_catalog(skills)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for AmpPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Amp
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        // Amp's documented precedence puts user-owned Agent/Amp roots ahead
        // of project roots, while the Claude-compatible project root still
        // wins over the user Claude root. Keep this provider-specific order
        // instead of using the generic project-first helper.
        let mut roots = vec![
            SkillRoot::new(context.home.join(".config/agents/skills"), SkillScope::User),
            SkillRoot::new(context.home.join(".agents/skills"), SkillScope::User),
            SkillRoot::new(context.home.join(".config/amp/skills"), SkillScope::User),
        ];
        if !is_global_context(context) {
            for dir in cwd_to_boundary(&context.cwd, context.repository_root.as_deref()) {
                roots.push(SkillRoot::new(
                    dir.join(".agents/skills"),
                    SkillScope::Repository,
                ));
                roots.push(SkillRoot::new(
                    dir.join(".claude/skills"),
                    SkillScope::Repository,
                ));
            }
        }
        roots.push(SkillRoot::new(
            context.home.join(".claude/skills"),
            SkillScope::User,
        ));
        roots.extend(amp_configured_skill_roots(context));
        roots
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        compatible_skill_is_valid(metadata)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        // Current Amp no longer exposes user-invokable Skills, so Claude's
        // disable-model-invocation extension cannot create a manual entry.
        SkillVisibility::Automatic
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        render_name_description_catalog(skills)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for DroidPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Droid
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        compatible_filesystem_roots(
            context,
            &[".factory/skills", ".agents/skills", ".agent/skills"],
            &[".factory/skills", ".agents/skills", ".agent/skills"],
            None,
        )
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        compatible_skill_is_valid(metadata)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        frontmatter_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        render_name_description_catalog(skills)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for PiAdapter {
    fn agent(&self) -> AgentKind {
        AgentKind::Pi
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let project_trusted = !is_global_context(context) && pi_project_is_trusted(context);
        let (project_configured, user_configured) =
            pi_configured_skill_roots(context, project_trusted);
        let mut roots = Vec::new();
        roots.extend(project_configured);
        if project_trusted {
            roots.push(
                SkillRoot::new(context.cwd.join(".pi/skills"), SkillScope::Local)
                    .with_root_markdown(),
            );
            let dirs = context
                .repository_root
                .as_deref()
                .map(|root| cwd_to_boundary(&context.cwd, Some(root)))
                .unwrap_or_else(|| context.cwd.ancestors().map(Path::to_path_buf).collect());
            let user_agents = fs::canonicalize(context.home.join(".agents/skills"))
                .unwrap_or_else(|_| context.home.join(".agents/skills"));
            for dir in dirs {
                let path = dir.join(".agents/skills");
                let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                if canonical == user_agents {
                    continue;
                }
                roots.push(SkillRoot::new(path, SkillScope::Repository));
            }
        }
        roots.extend(user_configured);
        roots.push(
            SkillRoot::new(pi_agent_dir(context).join("skills"), SkillScope::User)
                .with_root_markdown(),
        );
        roots.push(SkillRoot::new(
            context.home.join(".agents/skills"),
            SkillScope::User,
        ));
        if project_trusted {
            roots.extend(pi_package_skill_roots(
                &context.cwd.join(".pi/settings.json"),
                &context.cwd.join(".pi/npm/node_modules"),
                SkillScope::Repository,
            ));
        }
        roots.extend(pi_package_skill_roots(
            &pi_agent_dir(context).join("settings.json"),
            &pi_agent_dir(context).join("npm/node_modules"),
            SkillScope::User,
        ));
        roots
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> bool {
        metadata.has_explicit_description && !metadata.description.trim().is_empty()
    }

    fn prompt_path_for_entry(
        &self,
        _root: &SkillRoot,
        _skill_dir: &Path,
        skill_file: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> Option<String> {
        Some(skill_file.display().to_string())
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        if frontmatter_visibility(metadata) == SkillVisibility::ManualOnly {
            SkillVisibility::ManualOnly
        } else {
            SkillVisibility::Automatic
        }
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        let mut lines = vec![
            "".to_string(),
            "".to_string(),
            "The following skills provide specialized instructions for specific tasks.".to_string(),
            "Use the read tool to load a skill's file when the task matches its description.".to_string(),
            "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".to_string(),
            "".to_string(),
            "<available_skills>".to_string(),
        ];
        for skill in skills {
            lines.push("  <skill>".to_string());
            lines.push(format!("    <name>{}</name>", xml_escape(&skill.name)));
            lines.push(format!(
                "    <description>{}</description>",
                xml_escape(&skill.description)
            ));
            lines.push(format!(
                "    <location>{}</location>",
                xml_escape(skill.prompt_path.as_deref().unwrap_or_default())
            ));
            lines.push("  </skill>".to_string());
        }
        lines.push("</available_skills>".to_string());
        lines.join("\n")
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for GrokAdapter {
    fn agent(&self) -> AgentKind {
        AgentKind::Grok
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let mut roots = Vec::new();
        if !is_global_context(context) {
            for dir in cwd_to_boundary(&context.cwd, context.repository_root.as_deref()) {
                let scope = if dir == context.cwd {
                    SkillScope::Local
                } else {
                    SkillScope::Repository
                };
                for config in [".grok", ".agents", ".claude", ".cursor"] {
                    roots.push(SkillRoot::new(dir.join(config).join("skills"), scope));
                    roots.push(
                        SkillRoot::new(dir.join(config).join("commands"), scope).flat_markdown(),
                    );
                }
            }
        }
        for config_root in [
            grok_home(context),
            context.home.join(".agents"),
            context.home.join(".claude"),
            context.home.join(".cursor"),
        ] {
            roots.push(SkillRoot::new(config_root.join("skills"), SkillScope::User));
            roots.push(
                SkillRoot::new(config_root.join("commands"), SkillScope::User).flat_markdown(),
            );
        }
        roots.extend(grok_configured_skill_roots(context));
        roots
    }

    fn effective_name(&self, _skill_dir: &Path, metadata: &SkillMetadata) -> String {
        normalize_grok_name(&metadata.name)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::KeepAll
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        let canonical = fs::canonicalize(&metadata.source_path)
            .unwrap_or_else(|_| metadata.source_path.clone());
        !grok_config_values(context, "ignore")
            .into_iter()
            .map(|value| resolve_agent_path(&value, &context.home, &grok_home(context)))
            .map(|path| fs::canonicalize(&path).unwrap_or(path))
            .any(|ignored| canonical.starts_with(ignored))
    }

    fn prompt_path_for_entry(
        &self,
        _root: &SkillRoot,
        _skill_dir: &Path,
        skill_file: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> Option<String> {
        Some(skill_file.display().to_string())
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> SkillVisibility {
        if grok_config_values(context, "disabled")
            .iter()
            .any(|name| normalize_grok_name(name) == normalize_grok_name(&metadata.name))
        {
            return SkillVisibility::ManualOnly;
        }
        frontmatter_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        catalog::render_grok_listing(skills, 400_000).text
    }

    fn render_initial_catalog(&self, skills: &[EffectiveSkill]) -> catalog::CatalogRender {
        catalog::render_grok_listing(skills, 400_000)
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for OpenClawPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::OpenClaw
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let state_dir = openclaw_state_dir(context);
        let config = openclaw_config(context);
        let mut roots = Vec::new();
        if !is_global_context(context) {
            roots.push(SkillRoot::new(
                context.cwd.join("skills"),
                SkillScope::Local,
            ));
            roots.push(SkillRoot::new(
                context.cwd.join(".agents/skills"),
                SkillScope::Local,
            ));
        }
        roots.push(SkillRoot::new(
            context.home.join(".agents/skills"),
            SkillScope::User,
        ));
        roots.push(SkillRoot::new(state_dir.join("skills"), SkillScope::User));
        roots.extend(
            openclaw_extra_skill_dirs(&config, context)
                .into_iter()
                .map(|path| SkillRoot::new(path, SkillScope::User)),
        );
        roots.extend(openclaw_plugin_roots(&config, &state_dir, context));
        roots
    }

    fn is_enabled(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        context: &DiscoveryContext,
    ) -> bool {
        !openclaw_disabled_skills(&openclaw_config(context)).contains(&metadata.name)
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        frontmatter_visibility(metadata)
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        let body = skills
            .iter()
            .map(|skill| {
                format!(
                    "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>",
                    xml_escape(&skill.name),
                    xml_escape(&skill.description)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        truncate_chars(
            &format!("<available_skills>\n{body}\n</available_skills>"),
            30_000,
        )
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

impl AgentSkillPolicy for HermesPolicy {
    fn agent(&self) -> AgentKind {
        AgentKind::Hermes
    }

    fn roots(&self, context: &DiscoveryContext) -> Vec<SkillRoot> {
        let hermes_home = hermes_home(context);
        let mut roots = vec![SkillRoot::new(hermes_home.join("skills"), SkillScope::User)];
        roots.extend(
            hermes_external_skill_dirs(context, &hermes_home)
                .into_iter()
                .map(|path| SkillRoot::new(path, SkillScope::User)),
        );
        roots
    }

    fn name_collision(&self) -> NameCollision {
        NameCollision::FirstWins
    }

    fn visibility(
        &self,
        _skill_dir: &Path,
        _metadata: &SkillMetadata,
        _context: &DiscoveryContext,
    ) -> SkillVisibility {
        // Hermes currently indexes ordinary Skills for the model and does
        // not implement disable-model-invocation for standalone Skills.
        SkillVisibility::Automatic
    }

    fn render_visible_metadata(&self, skills: &[EffectiveSkill]) -> String {
        if skills.is_empty() {
            return String::new();
        }
        let body = skills
            .iter()
            .map(|skill| {
                format!(
                    "- {}: {}",
                    skill.name,
                    truncate_chars(&skill.description, 57)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("<available_skills>\n{body}\n</available_skills>")
    }

    fn estimate_tokens(&self, rendered: &str) -> usize {
        approx_token_count(rendered)
    }
}

/// The desktop's Global project is represented by scanning `home` as the
/// selected path. It must not be treated as an ordinary project whose parent
/// directories contribute project-local compatibility roots.
fn is_global_context(context: &DiscoveryContext) -> bool {
    context.cwd == context.home
}

fn find_repository_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| dir.join(".git").is_dir() || dir.join(".git").is_file())
        .map(Path::to_path_buf)
}

fn codex_home(context: &DiscoveryContext) -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".codex"))
}

fn claude_config_dir(context: &DiscoveryContext) -> PathBuf {
    env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".claude"))
}

fn grok_home(context: &DiscoveryContext) -> PathBuf {
    env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".grok"))
}

fn grok_configured_skill_roots(context: &DiscoveryContext) -> Vec<SkillRoot> {
    let grok_home = grok_home(context);
    grok_config_values(context, "paths")
        .into_iter()
        .map(|value| resolve_agent_path(&value, &context.home, &grok_home))
        .map(|path| {
            if path.is_file() {
                SkillRoot::exact(path, SkillScope::User)
            } else {
                SkillRoot::new(path, SkillScope::User)
            }
        })
        .collect()
}

fn grok_config_values(context: &DiscoveryContext, key: &str) -> Vec<String> {
    let Ok(config) = fs::read_to_string(grok_home(context).join("config.toml")) else {
        return Vec::new();
    };
    let mut in_skills = false;
    let mut values = Vec::new();
    for raw_line in config.lines() {
        let line = raw_line.trim();
        if line == "[skills]" {
            in_skills = true;
            continue;
        }
        if line.starts_with('[') {
            in_skills = false;
            continue;
        }
        if !in_skills
            || !line
                .split_once('=')
                .is_some_and(|(candidate, _)| candidate.trim() == key)
        {
            continue;
        }
        let Some((_, value)) = line.split_once('=') else {
            continue;
        };
        values.extend(
            value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(str::trim)
                .filter_map(|value| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
                .map(str::to_string),
        );
    }
    values
}

fn normalize_grok_name(name: &str) -> String {
    let mut normalized = String::new();
    let mut previous_dash = false;
    for character in name.trim().to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character);
            previous_dash = false;
        } else if !previous_dash && !normalized.is_empty() {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_end_matches('-').chars().take(64).collect()
}

fn pi_agent_dir(context: &DiscoveryContext) -> PathBuf {
    env::var_os("PI_CODING_AGENT_DIR")
        .map(|value| resolve_agent_path(&value.to_string_lossy(), &context.home, &context.home))
        .unwrap_or_else(|| context.home.join(".pi/agent"))
}

fn pi_project_is_trusted(context: &DiscoveryContext) -> bool {
    let Ok(content) = fs::read_to_string(pi_agent_dir(context).join("trust.json")) else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return true;
    };
    let Some(entries) = value.as_object() else {
        return true;
    };
    let mut current = fs::canonicalize(&context.cwd).unwrap_or_else(|_| context.cwd.clone());
    loop {
        if let Some(decision) = entries.get(&current.to_string_lossy().to_string()) {
            return decision.as_bool().unwrap_or(true);
        }
        let Some(parent) = current.parent() else {
            return true;
        };
        if parent == current {
            return true;
        }
        current = parent.to_path_buf();
    }
}

fn pi_configured_skill_roots(
    context: &DiscoveryContext,
    project_trusted: bool,
) -> (Vec<SkillRoot>, Vec<SkillRoot>) {
    let mut project_roots = Vec::new();
    let mut user_roots = Vec::new();
    if project_trusted {
        project_roots.extend(pi_settings_skill_roots(
            &context.cwd.join(".pi/settings.json"),
            &context.cwd.join(".pi"),
            context,
            SkillScope::Repository,
        ));
    }
    let agent_dir = pi_agent_dir(context);
    user_roots.extend(pi_settings_skill_roots(
        &agent_dir.join("settings.json"),
        &agent_dir,
        context,
        SkillScope::User,
    ));
    (project_roots, user_roots)
}

fn pi_package_skill_roots(
    settings_path: &Path,
    install_root: &Path,
    scope: SkillScope,
) -> Vec<SkillRoot> {
    let Ok(content) = fs::read_to_string(settings_path) else {
        return Vec::new();
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&normalize_jsonc(&content)) else {
        return Vec::new();
    };
    let Some(packages) = settings.get("packages").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for package in packages {
        let source = package
            .as_str()
            .or_else(|| package.get("source").and_then(|value| value.as_str()));
        let Some(package_name) = source.and_then(pi_npm_package_name) else {
            continue;
        };
        let package_root = install_root.join(package_name);
        let manifest = fs::read_to_string(package_root.join("package.json"))
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok());
        let entries = manifest
            .as_ref()
            .and_then(|manifest| manifest.pointer("/pi/skills"))
            .and_then(|value| value.as_array());
        if let Some(entries) = entries {
            for entry in entries.iter().filter_map(|value| value.as_str()) {
                if entry.starts_with('!') || entry.contains('*') {
                    continue;
                }
                let path = package_root.join(entry);
                roots.push(if path.is_file() {
                    SkillRoot::exact(path, scope)
                } else {
                    SkillRoot::new(path, scope).with_root_markdown()
                });
            }
        } else {
            roots.push(SkillRoot::new(package_root.join("skills"), scope).with_root_markdown());
        }
    }
    roots
}

fn pi_npm_package_name(source: &str) -> Option<&str> {
    let source = source.strip_prefix("npm:").unwrap_or(source);
    if source.starts_with("git:") || source.contains("://") || source.starts_with("github:") {
        return None;
    }
    if source.starts_with('@') {
        let slash = source.find('/')?;
        let version = source[slash + 1..]
            .find('@')
            .map(|offset| slash + 1 + offset)
            .unwrap_or(source.len());
        Some(&source[..version])
    } else {
        Some(source.split('@').next().unwrap_or(source))
    }
}

fn pi_settings_skill_roots(
    settings_path: &Path,
    relative_base: &Path,
    context: &DiscoveryContext,
    scope: SkillScope,
) -> Vec<SkillRoot> {
    let Ok(content) = fs::read_to_string(settings_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&normalize_jsonc(&content)) else {
        return Vec::new();
    };
    let Some(entries) = value.get("skills") else {
        return Vec::new();
    };
    let entries = entries
        .as_array()
        .cloned()
        .or_else(|| {
            entries
                .get("customDirectories")
                .and_then(|value| value.as_array())
                .cloned()
        })
        .unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| entry.as_str())
        .filter(|entry| !entry.starts_with('!'))
        .map(|entry| {
            let path = resolve_agent_path(entry, &context.home, relative_base);
            if path.extension().is_some_and(|extension| extension == "md") {
                SkillRoot::exact(path, scope)
            } else {
                SkillRoot::new(path, scope).with_root_markdown()
            }
        })
        .collect()
}

fn resolve_agent_path(value: &str, home: &Path, relative_base: &Path) -> PathBuf {
    if value == "~" {
        return home.to_path_buf();
    }
    if let Some(relative) = value.strip_prefix("~/") {
        return home.join(relative);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        relative_base.join(path)
    }
}

fn compatible_filesystem_roots(
    context: &DiscoveryContext,
    project_relative: &[&str],
    user_relative: &[&str],
    extra_env: Option<&str>,
) -> Vec<SkillRoot> {
    let mut roots = Vec::new();
    if !is_global_context(context) {
        for dir in cwd_to_boundary(&context.cwd, context.repository_root.as_deref()) {
            for relative in project_relative {
                roots.push(SkillRoot::new(dir.join(relative), SkillScope::Repository));
            }
        }
    }
    for relative in user_relative {
        roots.push(SkillRoot::new(
            context.home.join(relative),
            SkillScope::User,
        ));
    }
    if let Some(name) = extra_env {
        if let Some(value) = env::var_os(name) {
            roots.extend(env::split_paths(&value).map(|path| {
                let path = if path.is_absolute() {
                    path
                } else {
                    context.cwd.join(path)
                };
                SkillRoot::new(path, SkillScope::User).with_root_markdown()
            }));
        }
    }
    roots
}

fn amp_configured_skill_roots(context: &DiscoveryContext) -> Vec<SkillRoot> {
    let mut paths = Vec::new();
    if let Some(value) = env::var_os("AMP_SKILL_PATHS") {
        paths.extend(env::split_paths(&value).map(|path| {
            if path.is_absolute() {
                path
            } else {
                context.cwd.join(path)
            }
        }));
    }
    let config_files = [
        context.home.join(".config/amp/settings.json"),
        context.home.join(".config/amp/config.json"),
        context.cwd.join(".amp/settings.json"),
        context.cwd.join(".amp/config.json"),
    ];
    for file in config_files {
        if is_global_context(context) && file.starts_with(&context.cwd.join(".amp")) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&normalize_jsonc(&content))
        else {
            continue;
        };
        let Some(skill_value) = value
            .pointer("/skills/path")
            .or_else(|| value.pointer("/skills/paths"))
            .or_else(|| value.get("skillPath"))
        else {
            continue;
        };
        let values = skill_value
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| skill_value.as_str().into_iter().collect::<Vec<_>>());
        paths.extend(values.into_iter().map(|path| {
            resolve_agent_path(path, &context.home, file.parent().unwrap_or(&context.cwd))
        }));
    }
    paths
        .into_iter()
        .map(|path| SkillRoot::new(path, SkillScope::User).with_root_markdown())
        .collect()
}

fn compatible_skill_is_valid(metadata: &SkillMetadata) -> bool {
    metadata.has_explicit_description
        && !metadata.description.trim().is_empty()
        && metadata.description.encode_utf16().count() <= 1_024
        && valid_agent_skill_name(&metadata.name)
}

fn render_name_description_catalog(skills: &[EffectiveSkill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let body = skills
        .iter()
        .map(|skill| {
            format!(
                "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>",
                xml_escape(&skill.name),
                xml_escape(&skill.description)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<available_skills>\n{body}\n</available_skills>")
}

fn path_derived_skill_id(root: &SkillRoot, skill_dir: &Path, skill_file: &Path) -> String {
    if let Ok(relative) = skill_dir.strip_prefix(&root.path) {
        if let Some(name) = relative.file_name() {
            return name.to_string_lossy().into_owned();
        }
    }
    skill_file
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "SKILL".to_string())
}

fn opencode_visibility(metadata: &SkillMetadata) -> SkillVisibility {
    if !metadata.has_explicit_description || metadata.description.trim().is_empty() {
        return SkillVisibility::ManualOnly;
    }
    let Ok(content) = fs::read_to_string(&metadata.source_path) else {
        return SkillVisibility::Automatic;
    };
    let Some(frontmatter) = yaml_frontmatter(&content) else {
        return SkillVisibility::Automatic;
    };
    let Ok(parsed) = serde_yaml::from_str::<SkillFrontmatter>(frontmatter) else {
        return SkillVisibility::Automatic;
    };
    if yaml_opencode_autoinvoke_disabled(parsed.metadata.as_ref()) {
        SkillVisibility::ManualOnly
    } else {
        SkillVisibility::Automatic
    }
}

fn yaml_opencode_autoinvoke_disabled(value: Option<&serde_yaml::Value>) -> bool {
    let Some(value) = value else { return false };
    if value
        .get("opencode/autoinvoke")
        .is_some_and(|value| value.as_bool() == Some(false) || value.as_str() == Some("false"))
    {
        return true;
    }
    let Some(opencode) = value.get("opencode") else {
        return false;
    };
    let Some(autoinvoke) = opencode.get("autoinvoke") else {
        return false;
    };
    autoinvoke.as_bool() == Some(false) || autoinvoke.as_str() == Some("false")
}

fn openclaw_state_dir(context: &DiscoveryContext) -> PathBuf {
    env::var_os("OPENCLAW_STATE_DIR")
        .or_else(|| env::var_os("CLAWDBOT_STATE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".openclaw"))
}

fn openclaw_config(context: &DiscoveryContext) -> serde_json::Value {
    let state_dir = openclaw_state_dir(context);
    let path = env::var_os("OPENCLAW_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_dir.join("openclaw.json"));
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&normalize_jsonc(&content)).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn openclaw_extra_skill_dirs(
    config: &serde_json::Value,
    context: &DiscoveryContext,
) -> Vec<PathBuf> {
    config
        .pointer("/skills/load/extraDirs")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(|path| resolve_agent_path(path, &context.home, &context.cwd))
        .collect()
}

fn openclaw_disabled_skills(config: &serde_json::Value) -> HashSet<String> {
    config
        .pointer("/skills/entries")
        .and_then(|value| value.as_object())
        .into_iter()
        .flatten()
        .filter_map(|(name, entry)| {
            (entry.get("enabled").and_then(|value| value.as_bool()) == Some(false))
                .then(|| name.clone())
        })
        .collect()
}

fn openclaw_plugin_roots(
    config: &serde_json::Value,
    state_dir: &Path,
    context: &DiscoveryContext,
) -> Vec<SkillRoot> {
    if config
        .pointer("/plugins/enabled")
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        return Vec::new();
    }
    let allow = config
        .pointer("/plugins/allow")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string)
                .collect::<HashSet<_>>()
        });
    let deny = config
        .pointer("/plugins/deny")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(state_dir.join("extensions")) {
        candidates.extend(entries.flatten().map(|entry| entry.path()));
    }
    if let Some(paths) = config
        .pointer("/plugins/load/paths")
        .and_then(|value| value.as_array())
    {
        candidates.extend(
            paths
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| resolve_agent_path(value, &context.home, state_dir)),
        );
    }
    let mut seen = HashSet::new();
    let mut roots = Vec::new();
    for candidate in candidates {
        let plugin_root = if candidate.is_file() {
            candidate.parent().unwrap_or(&candidate).to_path_buf()
        } else {
            candidate
        };
        let canonical = fs::canonicalize(&plugin_root).unwrap_or_else(|_| plugin_root.clone());
        if !seen.insert(canonical) {
            continue;
        }
        let manifest_path = plugin_root.join("openclaw.plugin.json");
        let Ok(content) = fs::read_to_string(manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let Some(id) = manifest.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let entry_disabled = config
            .get("plugins")
            .and_then(|plugins| plugins.get("entries"))
            .and_then(|entries| entries.get(id))
            .and_then(|entry| entry.get("enabled"))
            .and_then(|value| value.as_bool())
            == Some(false);
        if deny.contains(id)
            || allow.as_ref().is_some_and(|allow| !allow.contains(id))
            || entry_disabled
        {
            continue;
        }
        let display_name = manifest
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or(id);
        let skill_paths = manifest
            .get("skills")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str());
        for relative in skill_paths {
            let relative = PathBuf::from(relative);
            if !relative.is_absolute()
                && !relative.components().any(|part| part.as_os_str() == "..")
            {
                roots.push(SkillRoot::plugin(
                    plugin_root.join(relative),
                    SkillScope::User,
                    id,
                    display_name,
                ));
            }
        }
    }
    roots
}

fn hermes_home(context: &DiscoveryContext) -> PathBuf {
    env::var_os("HERMES_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".hermes"))
}

fn hermes_external_skill_dirs(context: &DiscoveryContext, hermes_home: &Path) -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(hermes_home.join("config.yaml")) else {
        return Vec::new();
    };
    let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return Vec::new();
    };
    config
        .get("skills")
        .and_then(|skills| skills.get("external_dirs"))
        .and_then(|value| value.as_sequence())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(|path| resolve_agent_path(path, &context.home, hermes_home))
        .collect()
}

fn claude_skill_override(context: &DiscoveryContext, name: &str) -> Option<String> {
    claude_skill_overrides(context).remove(name)
}

fn claude_settings_files(context: &DiscoveryContext) -> Vec<PathBuf> {
    let project_root = context.repository_root.as_deref().unwrap_or(&context.cwd);
    let mut files = vec![context.home.join(".claude/settings.json")];
    files.extend([
        project_root.join(".claude/settings.json"),
        project_root.join(".claude/settings.local.json"),
    ]);
    #[cfg(target_os = "macos")]
    files.push(PathBuf::from(
        "/Library/Application Support/ClaudeCode/managed-settings.json",
    ));
    #[cfg(all(unix, not(target_os = "macos")))]
    files.push(PathBuf::from("/etc/claude-code/managed-settings.json"));
    files
}

fn claude_skill_overrides(context: &DiscoveryContext) -> HashMap<String, String> {
    let mut overrides = HashMap::new();
    for file in claude_settings_files(context) {
        let Ok(content) = fs::read_to_string(file) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let Some(entries) = value
            .get("skillOverrides")
            .and_then(|value| value.as_object())
        else {
            continue;
        };
        for (name, state) in entries {
            if let Some(state) = state.as_str() {
                overrides.insert(name.clone(), state.to_string());
            }
        }
    }
    overrides
}

fn claude_plugin_roots(context: &DiscoveryContext) -> Vec<SkillRoot> {
    let mut enabled = HashSet::new();
    for file in claude_settings_files(context) {
        let Ok(content) = fs::read_to_string(file) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        if let Some(plugins) = value
            .get("enabledPlugins")
            .and_then(|value| value.as_object())
        {
            for (id, state) in plugins {
                if state.as_bool() == Some(true) {
                    enabled.insert(id.clone());
                } else if state.as_bool() == Some(false) {
                    enabled.remove(id);
                }
            }
        }
    }
    let path = context.home.join(".claude/plugins/installed_plugins.json");
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };
    let Some(plugins) = value.get("plugins").and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    let project_root = context.repository_root.as_deref().unwrap_or(&context.cwd);
    let global_only = is_global_context(context);
    let mut roots = Vec::new();
    for (id, installations) in plugins {
        if !enabled.contains(id) {
            continue;
        }
        let Some(installations) = installations.as_array() else {
            continue;
        };
        for installation in installations {
            let scope = installation.get("scope").and_then(|value| value.as_str());
            let applies = scope == Some("user")
                || (!global_only
                    && scope == Some("project")
                    && installation
                        .get("projectPath")
                        .and_then(|value| value.as_str())
                        .is_some_and(|path| Path::new(path) == project_root));
            if !applies {
                continue;
            }
            if let Some(path) = installation
                .get("installPath")
                .and_then(|value| value.as_str())
            {
                let scope = match scope {
                    Some("project") => SkillScope::Repository,
                    Some("user") => SkillScope::User,
                    _ => SkillScope::System,
                };
                let plugin_name = plugin_display_fallback(id);
                roots.extend(plugin_skill_roots(
                    Path::new(path),
                    ".claude-plugin/plugin.json",
                    id,
                    "skills",
                    scope,
                    plugin_name,
                ));
            }
        }
    }
    roots
}

fn claude_plugin_name(skill_dir: &Path) -> Option<String> {
    let components = skill_dir
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    components
        .windows(5)
        .find(|parts| parts[0] == ".claude" && parts[1] == "plugins" && parts[2] == "cache")
        .map(|parts| parts[4].clone())
}

fn claude_local_name(skill_dir: &Path, metadata: &SkillMetadata) -> String {
    if metadata
        .source_path
        .file_name()
        .is_some_and(|name| name != "SKILL.md")
    {
        return metadata.name.clone();
    }
    skill_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.name.clone())
}

#[derive(Default)]
struct OpenCodeConfigSnapshot {
    skill_paths: Vec<(PathBuf, SkillScope)>,
    permission_rules: Vec<(String, String)>,
}

fn opencode_config_home(context: &DiscoveryContext) -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".config"))
        .join("opencode")
}

fn env_truthy(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn opencode_config(context: &DiscoveryContext) -> OpenCodeConfigSnapshot {
    let config_home = opencode_config_home(context);
    let mut files = vec![
        (config_home.join("opencode.json"), SkillScope::User),
        (config_home.join("opencode.jsonc"), SkillScope::User),
    ];
    if let Some(custom) = env::var_os("OPENCODE_CONFIG").map(PathBuf::from) {
        files.push((custom, SkillScope::User));
    }
    if !is_global_context(context) {
        let project_configs = cwd_to_boundary(&context.cwd, context.repository_root.as_deref())
            .into_iter()
            .rev()
            .flat_map(|dir| [dir.join("opencode.json"), dir.join("opencode.jsonc")])
            .filter(|path| path.is_file());
        files.extend(project_configs.map(|path| (path, SkillScope::Repository)));
        if let Some(project_config) = env::var_os("OPENCODE_PROJECT_CONFIG").map(PathBuf::from) {
            files.push((project_config, SkillScope::Repository));
        }
    }
    #[cfg(target_os = "macos")]
    files.extend([
        (
            PathBuf::from("/Library/Application Support/opencode/opencode.json"),
            SkillScope::System,
        ),
        (
            PathBuf::from("/Library/Application Support/opencode/opencode.jsonc"),
            SkillScope::System,
        ),
    ]);

    let mut snapshot = OpenCodeConfigSnapshot::default();
    let mut default_agent = "build".to_string();
    let mut values = Vec::new();
    for (file, scope) in files {
        let Ok(content) = fs::read_to_string(file) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&normalize_jsonc(&content))
        else {
            continue;
        };
        if let Some(agent) = value.get("default_agent").and_then(|value| value.as_str()) {
            default_agent = agent.to_string();
        }
        values.push((value, scope));
    }
    for (value, scope) in values {
        let skill_paths = value
            .get("skills")
            .and_then(|skills| skills.as_array())
            .or_else(|| {
                value
                    .get("skills")
                    .and_then(|skills| skills.get("paths"))
                    .and_then(|paths| paths.as_array())
            });
        if let Some(paths) = skill_paths {
            for path in paths.iter().filter_map(|value| value.as_str()) {
                if !path.starts_with("http://") && !path.starts_with("https://") {
                    snapshot
                        .skill_paths
                        .push((expand_user_path(path, context), scope));
                }
            }
        }
        append_opencode_permission_rules(
            value
                .get("permissions")
                .or_else(|| value.get("permission").and_then(|value| value.get("skill"))),
            &mut snapshot.permission_rules,
        );
        append_opencode_permission_rules(
            value
                .get("agents")
                .or_else(|| value.get("agent"))
                .and_then(|value| value.get(&default_agent))
                .and_then(|value| {
                    value
                        .get("permissions")
                        .or_else(|| value.get("permission").and_then(|value| value.get("skill")))
                }),
            &mut snapshot.permission_rules,
        );
    }
    snapshot
}

fn append_opencode_permission_rules(
    value: Option<&serde_json::Value>,
    rules: &mut Vec<(String, String)>,
) {
    let Some(value) = value else { return };
    if let Some(action) = value.as_str() {
        rules.push(("*".to_string(), action.to_string()));
    } else if let Some(entries) = value.as_array() {
        for entry in entries {
            if entry
                .get("action")
                .and_then(|value| value.as_str())
                .is_some_and(|action| action != "skill")
            {
                continue;
            }
            let Some(resource) = entry
                .get("resource")
                .or_else(|| entry.get("pattern"))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let Some(effect) = entry
                .get("effect")
                .or_else(|| entry.get("action"))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            rules.push((resource.to_string(), effect.to_string()));
        }
    } else if let Some(entries) = value.as_object() {
        for (pattern, action) in entries {
            if let Some(action) = action.as_str() {
                rules.push((pattern.clone(), action.to_string()));
            }
        }
    }
}

fn opencode_skill_permission(context: &DiscoveryContext, name: &str) -> String {
    let mut action = "allow".to_string();
    for (pattern, candidate) in opencode_config(context).permission_rules {
        if wildcard_match(&pattern, name) {
            action = candidate;
        }
    }
    action
}

fn expand_user_path(value: &str, context: &DiscoveryContext) -> PathBuf {
    if let Some(relative) = value.strip_prefix("~/") {
        context.home.join(relative)
    } else {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            context.cwd.join(path)
        }
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut rest = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(position) = rest.find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && position != 0 {
            return false;
        }
        rest = &rest[position + part.len()..];
    }
    pattern.ends_with('*') || rest.is_empty()
}

fn normalize_jsonc(content: &str) -> String {
    let mut without_comments = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while let Some(ch) = chars.next() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
                without_comments.push(ch);
            }
            continue;
        }
        if block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if in_string {
            without_comments.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            without_comments.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            line_comment = true;
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block_comment = true;
        } else {
            without_comments.push(ch);
        }
    }

    let mut normalized = String::with_capacity(without_comments.len());
    let chars = without_comments.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let ch = chars[index];
        if in_string {
            normalized.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            normalized.push(ch);
            index += 1;
            continue;
        }
        if ch == ',' {
            let mut next = index + 1;
            while next < chars.len() && chars[next].is_whitespace() {
                next += 1;
            }
            if next < chars.len() && matches!(chars[next], '}' | ']') {
                index += 1;
                continue;
            }
        }
        normalized.push(ch);
        index += 1;
    }
    normalized
}

fn plugin_skill_roots(
    plugin_root: &Path,
    manifest_relative_path: &str,
    plugin_id: &str,
    fallback_skill_path: &str,
    scope: SkillScope,
    fallback_display_name: &str,
) -> Vec<SkillRoot> {
    let manifest_path = plugin_root.join(manifest_relative_path);
    let (display_name, skill_paths) = if manifest_path.is_file() {
        let Ok(content) = fs::read_to_string(&manifest_path) else {
            return Vec::new();
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Vec::new();
        };
        let display_name = manifest
            .get("interface")
            .and_then(|interface| interface.get("displayName"))
            .or_else(|| manifest.get("displayName"))
            .or_else(|| manifest.get("display_name"))
            .or_else(|| manifest.get("name"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback_display_name)
            .to_string();
        let skills_value = manifest
            .get("paths")
            .and_then(|paths| paths.get("skills"))
            .or_else(|| manifest.get("skills"));
        let paths = match skills_value {
            None => vec![fallback_skill_path.to_string()],
            Some(serde_json::Value::String(path)) => vec![path.clone()],
            Some(serde_json::Value::Array(paths)) => paths
                .iter()
                .filter_map(|path| path.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>(),
            // An explicitly empty or malformed contribution is not an
            // invitation to scan the entire plugin cache.
            Some(_) => Vec::new(),
        };
        (display_name, paths)
    } else {
        (
            fallback_display_name.to_string(),
            vec![fallback_skill_path.to_string()],
        )
    };
    let source = SkillSource::Plugin {
        id: plugin_id.to_string(),
        display_name,
    };
    skill_paths
        .into_iter()
        .filter_map(|relative| {
            let relative = PathBuf::from(relative);
            if relative.is_absolute() {
                return None;
            }
            let path = plugin_root.join(relative);
            if path.is_file() {
                Some(SkillRoot::exact(path, scope).with_source(source.clone()))
            } else {
                Some(SkillRoot::new(path, scope).with_source(source.clone()))
            }
        })
        .collect()
}

fn plugin_display_fallback(plugin_id: &str) -> &str {
    plugin_id
        .split_once('@')
        .map(|(plugin, _)| plugin)
        .unwrap_or(plugin_id)
}

fn codex_plugin_roots(codex_home: &Path) -> Vec<SkillRoot> {
    let Ok(config) = fs::read_to_string(codex_home.join("config.toml")) else {
        return Vec::new();
    };
    enabled_plugin_ids(&config)
        .into_iter()
        .filter_map(|id| {
            let (plugin, marketplace) = id.rsplit_once('@')?;
            let versions = codex_home
                .join("plugins/cache")
                .join(marketplace)
                .join(plugin);
            let version = fs::read_dir(versions)
                .ok()?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .max_by_key(|entry| entry.file_name())?;
            let plugin_root = version.path();
            let display_name = plugin_display_fallback(&plugin);
            Some(plugin_skill_roots(
                &plugin_root,
                ".codex-plugin/plugin.json",
                &id,
                "skills",
                SkillScope::System,
                display_name,
            ))
        })
        .flatten()
        .map(SkillRoot::direct_children)
        .collect()
}

fn enabled_plugin_ids(config: &str) -> Vec<String> {
    let mut current = None;
    let mut enabled = Vec::new();
    for raw_line in config.lines() {
        let line = raw_line.trim();
        if let Some(value) = line
            .strip_prefix("[plugins.\"")
            .and_then(|value| value.strip_suffix("\"]"))
        {
            current = Some(value.to_string());
        } else if line.starts_with('[') {
            current = None;
        } else if line == "enabled = true" {
            if let Some(id) = current.take() {
                enabled.push(id);
            }
        }
    }
    enabled
}

fn codex_disabled_skills(codex_home: &Path, home: &Path) -> HashSet<PathBuf> {
    let Ok(config) = fs::read_to_string(codex_home.join("config.toml")) else {
        return HashSet::new();
    };
    let mut in_skill = false;
    let mut path = None;
    let mut disabled = HashSet::new();
    for raw_line in config.lines() {
        let line = raw_line.trim();
        if line == "[[skills.config]]" {
            in_skill = true;
            path = None;
        } else if line.starts_with('[') {
            in_skill = false;
            path = None;
        } else if in_skill {
            if let Some(value) = line
                .strip_prefix("path = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                path = Some(PathBuf::from(value));
            } else if line == "enabled = false" {
                if let Some(path) = path.take() {
                    let path = resolve_agent_path(&path.to_string_lossy(), home, codex_home);
                    disabled.insert(fs::canonicalize(&path).unwrap_or(path));
                }
            }
        }
    }
    disabled
}

#[cfg(test)]
fn skill_config_entries(config: &str) -> Vec<(PathBuf, bool)> {
    let mut entries = Vec::new();
    let mut in_skill = false;
    let mut path = None;
    let mut enabled = true;
    let flush = |entries: &mut Vec<(PathBuf, bool)>, path: &mut Option<PathBuf>, enabled: bool| {
        if let Some(path) = path.take() {
            entries.push((path, enabled));
        }
    };
    for raw_line in config.lines() {
        let line = raw_line.trim();
        if line == "[[skills.config]]" {
            if in_skill {
                flush(&mut entries, &mut path, enabled);
            }
            in_skill = true;
            enabled = true;
        } else if line.starts_with('[') {
            if in_skill {
                flush(&mut entries, &mut path, enabled);
            }
            in_skill = false;
        } else if in_skill {
            if let Some(value) = line
                .strip_prefix("path = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                path = Some(PathBuf::from(value));
            } else if line == "enabled = false" {
                enabled = false;
            } else if line == "enabled = true" {
                enabled = true;
            }
        }
    }
    if in_skill {
        flush(&mut entries, &mut path, enabled);
    }
    entries
}

fn cwd_to_boundary(cwd: &Path, boundary: Option<&Path>) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for dir in cwd.ancestors() {
        result.push(dir.to_path_buf());
        if boundary.is_some_and(|root| root == dir) {
            break;
        }
    }
    result
}

fn read_metadata(path: &Path) -> Option<SkillMetadata> {
    read_metadata_with_profile(path, MetadataProfile::StrictFrontmatter)
}

fn read_metadata_with_profile(path: &Path, profile: MetadataProfile) -> Option<SkillMetadata> {
    let content = fs::read_to_string(path).ok()?;
    let fallback = if path.file_name().is_some_and(|name| name == "SKILL.md") {
        path.parent()?.file_name()?.to_string_lossy().into_owned()
    } else {
        path.file_stem()?.to_string_lossy().into_owned()
    };
    let parsed = yaml_frontmatter(&content)
        .and_then(|frontmatter| serde_yaml::from_str::<SkillFrontmatter>(frontmatter).ok());
    if matches!(profile, MetadataProfile::StrictFrontmatter) && parsed.is_none() {
        return None;
    }
    let parsed = parsed.unwrap_or_default();
    let has_explicit_description = parsed.description.is_some();
    Some(SkillMetadata {
        name: parsed
            .name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(fallback),
        description: parsed
            .description
            .unwrap_or_else(|| first_body_paragraph(&content)),
        has_explicit_description,
        when_to_use: parsed.when_to_use,
        source_path: path.to_path_buf(),
    })
}

fn first_body_paragraph(content: &str) -> String {
    let has_frontmatter = yaml_frontmatter(content).is_some();
    let mut delimiters = if has_frontmatter { 0 } else { 2 };
    let mut paragraph = Vec::new();
    for line in content.lines() {
        if has_frontmatter && line.trim() == "---" && delimiters < 2 {
            delimiters += 1;
            continue;
        }
        if delimiters < 2 {
            continue;
        }
        if line.trim().is_empty() {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        paragraph.push(line.trim());
    }
    paragraph.join(" ")
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

/// Lightweight, provider-independent estimate used when an exact tokenizer is
/// not available.  Every provider passes its final rendered catalog here, so
/// fixed prompt markup, escaping and path fields are included consistently.
fn approx_token_count(rendered: &str) -> usize {
    rendered.len().div_ceil(4)
}

fn valid_agent_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn yaml_frontmatter(content: &str) -> Option<&str> {
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))?;
    let lf = rest.find("\n---\n").map(|at| (at, 5));
    let crlf = rest.find("\r\n---\r\n").map(|at| (at, 9));
    let eof = rest.strip_suffix("\n---").map(|body| (body.len(), 4));
    let (end, _) = [lf, crlf, eof]
        .into_iter()
        .flatten()
        .min_by_key(|(at, _)| *at)?;
    Some(&rest[..end])
}

fn frontmatter_visibility(metadata: &SkillMetadata) -> SkillVisibility {
    let Ok(content) = fs::read_to_string(&metadata.source_path) else {
        return SkillVisibility::Automatic;
    };
    parse_visibility(&content)
}

fn parse_visibility(content: &str) -> SkillVisibility {
    let Some(frontmatter) = yaml_frontmatter(content) else {
        return SkillVisibility::Automatic;
    };
    let Ok(parsed) = serde_yaml::from_str::<SkillFrontmatter>(frontmatter) else {
        return SkillVisibility::Automatic;
    };
    if parsed.disable_model_invocation {
        SkillVisibility::ManualOnly
    } else if parsed.paths.as_ref().is_some_and(yaml_paths_configured) {
        SkillVisibility::Conditional
    } else {
        SkillVisibility::Automatic
    }
}

fn yaml_paths_configured(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => false,
        serde_yaml::Value::String(value) => !value.trim().is_empty(),
        serde_yaml::Value::Sequence(values) => !values.is_empty(),
        _ => true,
    }
}

fn codex_manual_only(skill_dir: &Path) -> bool {
    let path = skill_dir.join("agents/openai.yaml");
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    serde_yaml::from_str::<OpenAiMetadata>(&content)
        .ok()
        .and_then(|data| data.policy)
        .and_then(|policy| policy.allow_implicit_invocation)
        .is_some_and(|allowed| !allowed)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    when_to_use: Option<String>,
    #[serde(default, rename = "disable-model-invocation")]
    disable_model_invocation: bool,
    paths: Option<serde_yaml::Value>,
    metadata: Option<serde_yaml::Value>,
}

#[derive(Deserialize)]
struct OpenAiMetadata {
    policy: Option<OpenAiPolicy>,
}

#[derive(Deserialize)]
struct OpenAiPolicy {
    allow_implicit_invocation: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_skill(root: &Path, name: &str, extra: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test\n{extra}---\nBody"),
        )
        .unwrap();
    }

    #[test]
    fn manual_skills_are_discovered_but_not_counted() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("repo/subdir");
        fs::create_dir_all(project.join(".agents/skills")).unwrap();
        fs::create_dir_all(temp.path().join("repo/.git")).unwrap();
        write_skill(&project.join(".agents/skills"), "automatic", "");
        write_skill(
            &project.join(".agents/skills"),
            "manual",
            "disable-model-invocation: true\n",
        );
        let context = DiscoveryContext {
            cwd: project,
            home: temp.path().join("home"),
            repository_root: Some(temp.path().join("repo")),
        };
        let estimate = estimate_with_policy(&PiPolicy, &context);
        assert_eq!(estimate.discovered_count, 2);
        assert_eq!(estimate.model_visible_count, 1);
        assert_eq!(estimate.manual_only_count, 1);
    }

    #[test]
    fn codex_openai_policy_marks_manual_only() {
        let temp = tempdir().unwrap();
        let skill = temp.path().join("demo");
        write_skill(temp.path(), "demo", "");
        fs::create_dir_all(skill.join("agents")).unwrap();
        fs::write(
            skill.join("agents/openai.yaml"),
            "policy:\n  allow_implicit_invocation: false\n",
        )
        .unwrap();
        assert!(is_manual_skill(&skill));
    }

    #[test]
    fn invocation_extensions_are_only_applied_by_providers_that_support_them() {
        let temp = tempdir().unwrap();
        let skill = temp.path().join("manual");
        write_skill(
            temp.path(),
            "manual",
            "disable-model-invocation: true\nmetadata:\n  opencode:\n    autoinvoke: false\n",
        );
        let metadata = read_metadata(&skill.join("SKILL.md")).unwrap();
        let context = DiscoveryContext {
            cwd: temp.path().to_path_buf(),
            home: temp.path().join("home"),
            repository_root: None,
        };

        for policy in [
            &ClaudeCodePolicy as &dyn AgentSkillPolicy,
            &CursorPolicy,
            &OpenCodePolicy,
            &CopilotPolicy,
            &DroidPolicy,
            &PiPolicy,
            &GrokPolicy,
            &OpenClawPolicy,
        ] {
            assert_eq!(
                policy.visibility(&skill, &metadata, &context),
                SkillVisibility::ManualOnly,
                "{} should honor its manual invocation metadata",
                policy.agent().label()
            );
        }

        for policy in [
            &AntigravityPolicy as &dyn AgentSkillPolicy,
            &AmpPolicy,
            &HermesPolicy,
        ] {
            assert_eq!(
                policy.visibility(&skill, &metadata, &context),
                SkillVisibility::Automatic,
                "{} must not honor unsupported invocation metadata",
                policy.agent().label()
            );
        }
    }

    #[test]
    fn codex_token_estimate_is_not_capped_at_two_thousand() {
        let policy = CodexPolicy;
        let rendered = "x".repeat(8_001);

        assert_eq!(policy.estimate_tokens(&rendered), 2_001);
    }

    #[test]
    fn pi_uses_nearest_skill_when_names_collide() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let cwd = repo.join("nested");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(&cwd.join(".agents/skills"), "same", "");
        write_skill(&repo.join(".agents/skills"), "same", "");
        let context = DiscoveryContext {
            cwd: cwd.clone(),
            home: temp.path().join("home"),
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&PiPolicy, &context);
        assert_eq!(estimate.discovered_count, 1);
        assert!(estimate.skills[0].path.starts_with(cwd));
    }

    #[test]
    fn pi_only_uses_cwd_pi_directory_and_accepts_root_markdown() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let cwd = repo.join("nested");
        fs::create_dir_all(cwd.join(".pi/skills")).unwrap();
        fs::create_dir_all(repo.join(".pi/skills")).unwrap();
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(
            cwd.join(".pi/skills/direct.md"),
            "---\nname: direct\ndescription: Direct skill\n---\n",
        )
        .unwrap();
        fs::write(
            cwd.join(".pi/skills/second.md"),
            "---\nname: second\ndescription: Second skill\n---\n",
        )
        .unwrap();
        assert_eq!(
            scanner::scan(
                &SkillRoot::new(cwd.join(".pi/skills"), SkillScope::Local).with_root_markdown(),
                scanner::ScanProfile::PiIgnored,
            )
            .len(),
            2
        );
        assert!(read_metadata(&cwd.join(".pi/skills/direct.md")).is_some());
        write_skill(&repo.join(".pi/skills"), "parent-pi", "");
        let context = DiscoveryContext {
            cwd,
            home: temp.path().join("home"),
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&PiPolicy, &context);
        assert_eq!(estimate.discovered_count, 2);
        assert!(estimate.skills.iter().any(|skill| skill.name == "direct"));
        assert!(estimate.skills.iter().any(|skill| skill.name == "second"));
    }

    #[test]
    fn pi_loads_settings_paths_for_other_agent_skill_roots() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        let cwd = repo.join("src");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&cwd).unwrap();

        write_skill(&home.join(".claude/skills"), "user-claude", "");
        write_skill(&home.join(".codex/skills"), "user-codex", "");
        write_skill(&repo.join(".claude/skills"), "project-claude", "");
        write_skill(&repo.join(".agents/skills"), "shared", "");

        let agent_dir = home.join(".pi/agent");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(
            agent_dir.join("settings.json"),
            r#"{"skills":["~/.claude/skills","~/.codex/skills"]}"#,
        )
        .unwrap();
        fs::create_dir_all(cwd.join(".pi")).unwrap();
        fs::write(
            cwd.join(".pi/settings.json"),
            r#"{"skills":["../../.claude/skills"]}"#,
        )
        .unwrap();

        let context = DiscoveryContext {
            cwd,
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&PiPolicy, &context);
        for name in ["user-claude", "user-codex", "project-claude", "shared"] {
            assert!(
                estimate.skills.iter().any(|skill| skill.name == name),
                "Pi did not discover configured skill {name}"
            );
        }
    }

    #[test]
    fn pi_loads_static_skills_from_configured_packages() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let package = project.join(".pi/npm/node_modules/example-pi-package");
        fs::create_dir_all(project.join(".pi")).unwrap();
        fs::create_dir_all(package.join("resources/release")).unwrap();
        fs::write(
            project.join(".pi/settings.json"),
            r#"{"packages":["npm:example-pi-package@1.0.0"]}"#,
        )
        .unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"pi":{"skills":["resources/release"]}}"#,
        )
        .unwrap();
        fs::write(
            package.join("resources/release/SKILL.md"),
            "---\nname: package-release\ndescription: Release from package\n---\n",
        )
        .unwrap();

        let estimate = estimate_project_with_home(&project, &home)
            .into_iter()
            .find(|estimate| estimate.agent == AgentKind::Pi)
            .unwrap();
        assert!(
            estimate
                .skills
                .iter()
                .any(|skill| skill.name == "package-release")
        );
    }

    #[test]
    fn pi_requires_a_valid_description_and_uses_the_real_prompt_wrapper() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".pi/skills")).unwrap();
        fs::create_dir_all(project.join(".pi/skills/no-description")).unwrap();
        fs::write(
            project.join(".pi/skills/no-description/SKILL.md"),
            "---\nname: no-description\n---\nBody",
        )
        .unwrap();
        write_skill(&project.join(".pi/skills"), "valid", "");
        let context = DiscoveryContext {
            cwd: project,
            home: temp.path().join("home"),
            repository_root: None,
        };
        let estimate = estimate_with_policy(&PiPolicy, &context);
        assert!(
            estimate
                .skills
                .iter()
                .all(|skill| skill.name != "no-description")
        );
        let rendered = PiPolicy.render_visible_metadata(&estimate.skills);
        assert!(rendered.starts_with("\n\nThe following skills provide specialized instructions"));
        assert!(rendered.contains("<location>"));
        assert_eq!(estimate.estimated_tokens, approx_token_count(&rendered));
    }

    #[test]
    fn codex_config_parsers_only_keep_enabled_plugins_and_disabled_skills() {
        let temp = tempdir().unwrap();
        let skill = temp.path().join("demo/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "---\nname: demo\ndescription: Test\n---\n").unwrap();
        let config = format!(
            "[plugins.\"on@market\"]\nenabled = true\n\
             [plugins.\"off@market\"]\nenabled = false\n\
             [[skills.config]]\npath = \"{}\"\nenabled = false\n",
            skill.display()
        );
        fs::write(temp.path().join("config.toml"), config.clone()).unwrap();
        assert_eq!(enabled_plugin_ids(&config), vec!["on@market"]);
        assert!(
            codex_disabled_skills(temp.path(), temp.path())
                .contains(&fs::canonicalize(&skill).unwrap())
        );
        assert_eq!(skill_config_entries(&config), vec![(skill, false)]);
    }

    #[test]
    fn codex_enabled_skill_config_selects_but_does_not_add_a_root() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let external = temp.path().join("external");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(&project).unwrap();
        write_skill(&external, "configured-only", "");
        fs::write(
            home.join(".codex/config.toml"),
            format!(
                "[[skills.config]]\npath = \"{}\"\nenabled = true\n",
                external.display()
            ),
        )
        .unwrap();

        let estimate = estimate_project_with_home(&project, &home)
            .into_iter()
            .find(|estimate| estimate.agent == AgentKind::Codex)
            .unwrap();
        assert!(
            !estimate
                .skills
                .iter()
                .any(|skill| skill.name == "configured-only")
        );
    }

    #[test]
    fn claude_uses_direct_children_body_fallback_and_keeps_same_names() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let project_skill = project.join(".claude/skills/same");
        let user_skill = home.join(".claude/skills/same");
        let nested = project.join(".claude/skills/group/nested");
        fs::create_dir_all(&project_skill).unwrap();
        fs::create_dir_all(&user_skill).unwrap();
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            project_skill.join("SKILL.md"),
            "Project fallback description",
        )
        .unwrap();
        fs::write(user_skill.join("SKILL.md"), "User fallback description").unwrap();
        fs::write(nested.join("SKILL.md"), "Nested description").unwrap();

        let estimate = estimate_project_with_home(&project, &home)
            .into_iter()
            .find(|estimate| estimate.agent == AgentKind::ClaudeCode)
            .unwrap();
        assert_eq!(
            estimate
                .skills
                .iter()
                .filter(|skill| skill.name == "same")
                .count(),
            2
        );
        assert!(estimate.skills.iter().any(|skill| {
            skill.name == "same" && skill.description == "Project fallback description"
        }));
        assert!(
            !estimate
                .skills
                .iter()
                .any(|skill| skill.path == nested.join("SKILL.md"))
        );
    }

    #[test]
    fn grok_loads_flat_commands_and_renders_startup_listing_fields() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let commands = project.join(".grok/commands");
        fs::create_dir_all(&commands).unwrap();
        fs::create_dir_all(commands.join("nested")).unwrap();
        fs::write(commands.join("Release Notes.md"), "Prepare release notes").unwrap();
        fs::write(
            commands.join("nested/SKILL.md"),
            "---\nname: nested-command\ndescription: Must stay hidden\n---\n",
        )
        .unwrap();

        let estimate = estimate_project_with_home(&project, &home)
            .into_iter()
            .find(|estimate| estimate.agent == AgentKind::Grok)
            .unwrap();
        let command = estimate
            .skills
            .iter()
            .find(|skill| skill.name == "release-notes")
            .unwrap();
        let rendered = GrokPolicy.render_visible_metadata(std::slice::from_ref(command));
        assert!(rendered.starts_with("<system-reminder>"));
        assert!(rendered.contains("The following skills are available for use:"));
        assert!(rendered.contains("Absolute path:"));
        assert!(rendered.contains(&command.path.display().to_string()));
        assert!(
            !estimate
                .skills
                .iter()
                .any(|skill| skill.name == "nested-command")
        );
    }

    #[test]
    fn groups_agent_specific_names_for_the_same_filesystem_skill() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let commands = home.join(".claude/commands");
        fs::create_dir_all(&commands).unwrap();
        fs::create_dir_all(&project).unwrap();
        let command = commands.join("build_from_zero.md");
        fs::write(&command, "Build from zero").unwrap();

        let estimates = estimate_project_with_home(&project, &home);
        let groups = group_effective_skills(
            estimates
                .iter()
                .flat_map(|estimate| estimate.skills.iter().map(|skill| (estimate.agent, skill))),
        );
        let group = groups
            .iter()
            .find(|group| {
                group
                    .entries()
                    .iter()
                    .any(|(_, skill)| skill.path == command)
            })
            .unwrap();
        let names = group
            .entries()
            .iter()
            .map(|(_, skill)| skill.name.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(group.name(), "build_from_zero");
        assert_eq!(names, HashSet::from(["build_from_zero", "build-from-zero"]));
        assert!(
            group
                .entries()
                .iter()
                .any(|(agent, _)| *agent == AgentKind::ClaudeCode)
        );
        assert!(
            group
                .entries()
                .iter()
                .any(|(agent, _)| *agent == AgentKind::Grok)
        );
    }

    #[test]
    fn grok_rekeys_same_scope_frontmatter_collisions_by_directory() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        let skills = project.join(".grok/skills");
        for directory in ["original", "copied"] {
            let path = skills.join(directory);
            fs::create_dir_all(&path).unwrap();
            fs::write(
                path.join("SKILL.md"),
                "---\nname: shared-name\ndescription: Shared\n---\n",
            )
            .unwrap();
        }

        let estimate = estimate_project_with_home(&project, &home)
            .into_iter()
            .find(|estimate| estimate.agent == AgentKind::Grok)
            .unwrap();
        let names = estimate
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<HashSet<_>>();
        assert!(names.contains("shared-name"));
        assert!(names.contains("copied") || names.contains("original"));
    }

    #[test]
    fn codex_plugin_manifest_paths_and_source_are_preserved() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let codex_home = home.join(".codex");
        let plugin_root = codex_home.join("plugins/cache/market/deployer/1.0.0");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[plugins.\"deployer@market\"]\nenabled = true\n",
        )
        .unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"deployer","interface":{"displayName":"Deployer"},"paths":{"skills":["contributions/release"]}}"#,
        )
        .unwrap();
        write_skill(&plugin_root.join("contributions"), "release", "");
        write_skill(&plugin_root.join("skills"), "decoy", "");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&CodexPolicy, &context);
        let plugin_skills = estimate.plugin_skills().collect::<Vec<_>>();
        assert_eq!(plugin_skills.len(), 1);
        assert_eq!(plugin_skills[0].name, "release");
        assert_eq!(
            plugin_skills[0].path,
            plugin_root.join("contributions/release/SKILL.md")
        );
        assert_eq!(
            plugin_skills[0].source,
            SkillSource::Plugin {
                id: "deployer@market".to_string(),
                display_name: "Deployer".to_string(),
            }
        );
    }

    #[test]
    fn codex_reads_project_codex_skills_as_well_as_ancestor_agents_skills() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let cwd = repo.join("nested");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        write_skill(&repo.join(".codex/skills"), "codex-local", "");
        write_skill(&repo.join(".agents/skills"), "shared", "");
        let context = DiscoveryContext {
            cwd,
            home: temp.path().join("home"),
            repository_root: Some(repo),
        };

        let estimate = estimate_with_policy(&CodexPolicy, &context);
        assert!(
            estimate
                .skills
                .iter()
                .any(|skill| skill.name == "codex-local")
        );
        assert!(estimate.skills.iter().any(|skill| skill.name == "shared"));
    }

    #[test]
    fn project_estimates_are_derived_from_the_selected_path() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let empty_project = temp.path().join("empty-project");
        let project_with_skill = temp.path().join("project-with-skill");
        fs::create_dir_all(&empty_project).unwrap();
        fs::create_dir_all(&project_with_skill).unwrap();
        write_skill(
            &project_with_skill.join(".agents/skills"),
            "selected-project-only",
            "",
        );

        let empty = estimate_project_with_home(&empty_project, &home);
        let selected = estimate_project_with_home(&project_with_skill, &home);

        for agent in [
            AgentKind::Codex,
            AgentKind::OpenCode,
            AgentKind::Pi,
            AgentKind::Grok,
        ] {
            let empty_count = empty
                .iter()
                .find(|estimate| estimate.agent == agent)
                .unwrap()
                .discovered_count;
            let selected_estimate = selected
                .iter()
                .find(|estimate| estimate.agent == agent)
                .unwrap();
            assert_eq!(selected_estimate.discovered_count, empty_count + 1);
            assert!(
                selected_estimate
                    .skills
                    .iter()
                    .any(|skill| skill.name == "selected-project-only")
            );
        }
        assert!(!empty.iter().any(|estimate| estimate.agent.is_global_only()));
        assert!(
            !selected
                .iter()
                .any(|estimate| estimate.agent.is_global_only())
        );
    }

    #[test]
    fn global_estimates_do_not_include_parent_project_roots() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        write_skill(&temp.path().join(".agents/skills"), "parent-project", "");
        write_skill(&home.join(".agents/skills"), "user-skill", "");

        let estimates = estimate_project_with_home(&home, &home);
        for agent in [AgentKind::Codex, AgentKind::OpenCode, AgentKind::Pi] {
            let estimate = estimates.iter().find(|item| item.agent == agent).unwrap();
            assert!(
                estimate
                    .skills
                    .iter()
                    .any(|skill| skill.name == "user-skill")
            );
            assert!(
                !estimate
                    .skills
                    .iter()
                    .any(|skill| skill.name == "parent-project")
            );
        }
        assert_eq!(
            estimates
                .iter()
                .filter(|estimate| estimate.agent.is_global_only())
                .count(),
            2
        );
    }

    #[test]
    fn compatible_agents_scan_both_agents_and_claude_roots() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let home = temp.path().join("home");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(&repo.join(".agents/skills"), "shared-agents", "");
        write_skill(&repo.join(".claude/skills"), "shared-claude", "");

        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        for policy in [
            &CursorPolicy as &dyn AgentSkillPolicy,
            &CopilotPolicy as &dyn AgentSkillPolicy,
            &AmpPolicy as &dyn AgentSkillPolicy,
        ] {
            let estimate = estimate_with_policy(policy, &context);
            assert!(
                estimate
                    .skills
                    .iter()
                    .any(|skill| skill.name == "shared-agents")
            );
            assert!(
                estimate
                    .skills
                    .iter()
                    .any(|skill| skill.name == "shared-claude")
            );
        }
    }

    #[test]
    fn cursor_deduplicates_same_named_skills_across_compatible_roots() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let home = temp.path().join("home");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(&repo.join(".cursor/skills"), "same", "");
        write_skill(&repo.join(".agents/skills"), "same", "");
        write_skill(&repo.join(".claude/skills"), "same", "");

        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo.clone()),
        };
        let estimate = estimate_with_policy(&CursorPolicy, &context);

        assert_eq!(estimate.discovered_count, 1);
        assert_eq!(estimate.model_visible_count, 1);
        assert!(
            estimate.skills[0]
                .path
                .starts_with(repo.join(".cursor/skills"))
        );
    }

    #[test]
    fn every_effective_detector_has_a_matching_agent_badge() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let project_estimates = estimate_project_with_home(temp.path(), &home);
        let global_estimates = estimate_project_with_home(&home, &home);
        for estimate in project_estimates.into_iter().chain(global_estimates) {
            assert!(
                crate::agents::AGENT_ICON_ORDER
                    .iter()
                    .any(|agent| agent.id == estimate.agent.id()),
                "missing badge for {}",
                estimate.agent.id()
            );
        }
    }

    #[test]
    fn disabled_codex_plugin_does_not_appear_even_when_cached() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let codex_home = home.join(".codex");
        let plugin_root = codex_home.join("plugins/cache/market/off/1.0.0");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[plugins.\"off@market\"]\nenabled = false\n",
        )
        .unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"Off","skills":"skills"}"#,
        )
        .unwrap();
        write_skill(&plugin_root.join("skills"), "hidden", "");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&CodexPolicy, &context);
        assert!(!estimate.plugin_skills().any(|skill| skill.name == "hidden"));
    }

    #[test]
    fn claude_plugin_manifest_string_path_and_source_are_preserved() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let plugin_root = home.join("plugins/deployer");
        fs::create_dir_all(plugin_root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".claude/plugins")).unwrap();
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"deployer@market":true}}"#,
        )
        .unwrap();
        fs::write(
            home.join(".claude/plugins/installed_plugins.json"),
            format!(
                r#"{{"plugins":{{"deployer@market":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
                plugin_root.display()
            ),
        )
        .unwrap();
        fs::write(
            plugin_root.join(".claude-plugin/plugin.json"),
            r#"{"name":"Deployer UI","skills":"contributions/release"}"#,
        )
        .unwrap();
        write_skill(&plugin_root.join("contributions"), "release", "");
        write_skill(&plugin_root.join("skills"), "decoy", "");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&ClaudeCodePolicy, &context);
        let plugin_skills = estimate.plugin_skills().collect::<Vec<_>>();
        assert_eq!(plugin_skills.len(), 1);
        assert_eq!(plugin_skills[0].name, "Deployer UI:release");
        assert_eq!(plugin_skills[0].source.plugin_id(), Some("deployer@market"));
        assert_eq!(
            plugin_skills[0].source.plugin_display_name(),
            Some("Deployer UI")
        );
    }

    #[test]
    fn claude_keeps_same_named_paths_and_honors_visibility_overrides() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(&home.join(".claude/skills"), "same", "");
        write_skill(&repo.join(".claude/skills"), "same", "");
        write_skill(
            &repo.join(".claude/skills"),
            "manual-claude",
            "disable-model-invocation: true\n",
        );
        write_skill(&repo.join(".claude/skills"), "compact", "");
        fs::create_dir_all(repo.join(".claude")).unwrap();
        fs::write(
            repo.join(".claude/settings.local.json"),
            r#"{"skillOverrides":{"compact":"name-only"}}"#,
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home: home.clone(),
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&ClaudeCodePolicy, &context);
        assert_eq!(
            estimate
                .skills
                .iter()
                .filter(|skill| skill.name == "same")
                .count(),
            2
        );
        assert!(
            estimate
                .skills
                .iter()
                .filter(|skill| skill.name == "same")
                .any(|skill| skill.path.starts_with(&home))
        );
        assert!(
            estimate
                .skills
                .iter()
                .filter(|skill| skill.name == "same")
                .any(|skill| skill.path.starts_with(&context.cwd))
        );
        assert!(estimate.manual_only_count >= 1);
        assert!(estimate.name_only_count >= 1);
        assert_eq!(
            estimate
                .skills
                .iter()
                .find(|skill| skill.name == "manual-claude")
                .unwrap()
                .visibility,
            SkillVisibility::ManualOnly
        );
    }

    #[test]
    fn opencode_ignores_invocation_extension_but_honors_deny_permission() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(
            &repo.join(".opencode/skills"),
            "still-visible",
            "disable-model-invocation: true\n",
        );
        write_skill(&repo.join(".opencode/skills"), "hidden", "");
        fs::write(
            repo.join("opencode.jsonc"),
            "{ // comment\n \"permission\": { \"skill\": { \"hidden\": \"deny\", }, },\n}",
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&OpenCodePolicy, &context);
        assert!(estimate.skills.iter().any(|skill| {
            skill.name == "still-visible" && skill.visibility == SkillVisibility::Automatic
        }));
        assert!(!estimate.skills.iter().any(|skill| skill.name == "hidden"));
    }

    #[test]
    fn opencode_uses_v2_ids_and_skills_array_sources() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("team-skills")).unwrap();
        fs::write(
            repo.join("team-skills/release.md"),
            "---\nname: Release Guide\ndescription: Release workflow\nmetadata:\n  opencode:\n    autoinvoke: false\n---\nBody",
        )
        .unwrap();
        fs::write(
            repo.join("opencode.jsonc"),
            r#"{
                "skills": ["./team-skills"],
            }"#,
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&OpenCodePolicy, &context);
        let skill = estimate
            .skills
            .iter()
            .find(|skill| skill.name == "Release Guide")
            .unwrap();
        assert_eq!(skill.id, "release");
        assert_eq!(skill.scope, SkillScope::Repository);
        assert_eq!(skill.visibility, SkillVisibility::ManualOnly);
        assert!(
            !OpenCodePolicy
                .render_visible_metadata(&estimate.skills)
                .contains("<location>")
        );
    }

    #[test]
    fn opencode_v2_permissions_only_apply_skill_action_rules() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write_skill(&repo.join(".opencode/skills"), "keep", "");
        fs::write(
            repo.join("opencode.json"),
            r#"{
                "permissions": [
                    {"action":"edit","resource":"keep","effect":"deny"},
                    {"action":"skill","resource":"keep","effect":"deny"}
                ]
            }"#,
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: repo.clone(),
            home,
            repository_root: Some(repo),
        };
        let estimate = estimate_with_policy(&OpenCodePolicy, &context);
        assert!(!estimate.skills.iter().any(|skill| skill.name == "keep"));
    }

    #[test]
    fn openclaw_prefers_workspace_and_groups_enabled_plugin_skills() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        let state = home.join(".openclaw");
        fs::create_dir_all(&workspace).unwrap();
        write_skill(&workspace.join("skills"), "same", "");
        write_skill(&home.join(".agents/skills"), "same", "");
        let plugin = state.join("extensions/demo");
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join("openclaw.plugin.json"),
            r#"{"id":"demo","name":"Demo Plugin","skills":["skills"]}"#,
        )
        .unwrap();
        write_skill(&plugin.join("skills"), "from-plugin", "");
        fs::create_dir_all(&state).unwrap();
        fs::write(
            state.join("openclaw.json"),
            r#"{"plugins":{"enabled":true}}"#,
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: workspace.clone(),
            home,
            repository_root: None,
        };

        let estimate = estimate_with_policy(&OpenClawPolicy, &context);
        assert_eq!(
            estimate
                .skills
                .iter()
                .filter(|skill| skill.name == "same")
                .count(),
            1
        );
        assert!(
            estimate
                .skills
                .iter()
                .find(|skill| skill.name == "same")
                .unwrap()
                .path
                .starts_with(workspace)
        );
        let plugin_skill = estimate
            .plugin_skills()
            .find(|skill| skill.name == "from-plugin")
            .unwrap();
        assert_eq!(plugin_skill.source.plugin_id(), Some("demo"));
    }

    #[test]
    fn hermes_uses_home_and_explicit_external_dirs_only() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let hermes = home.join(".hermes");
        let project = temp.path().join("project");
        write_skill(&hermes.join("skills"), "local", "");
        write_skill(&hermes.join("shared"), "external", "");
        write_skill(&project.join(".agents/skills"), "not-implicit", "");
        fs::write(
            hermes.join("config.yaml"),
            "skills:\n  external_dirs:\n    - shared\n",
        )
        .unwrap();
        let context = DiscoveryContext {
            cwd: project,
            home,
            repository_root: None,
        };

        let estimate = estimate_with_policy(&HermesPolicy, &context);
        assert!(estimate.skills.iter().any(|skill| skill.name == "local"));
        assert!(estimate.skills.iter().any(|skill| skill.name == "external"));
        assert!(
            !estimate
                .skills
                .iter()
                .any(|skill| skill.name == "not-implicit")
        );
        assert!(
            !HermesPolicy
                .render_visible_metadata(&estimate.skills)
                .contains("SKILL.md")
        );
    }
}
