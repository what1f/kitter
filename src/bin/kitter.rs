use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use kitter::{
    InstallTarget, ProjectSkill, ProjectSkillInstallation, SkillGroup, SkillLibrary, SkillSummary,
    adoption,
    effective_skills::{self, AgentContextEstimate, AgentKind, group_effective_skills},
    project, source,
    tags::{TagId, TagState, load_tag_states, save_tag_states},
};
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "kitter",
    version,
    about = "Manage Agent Skills across projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List Skills saved in Kitter
    List {
        /// Only show Skills carrying this tag
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show one Skill by name or ID
    Show {
        skill: String,
        #[arg(long)]
        json: bool,
    },
    /// List files in one Skill
    Files { skill: String },
    /// Read one file from a Skill
    Read { skill: String, path: PathBuf },
    /// Add Skills from a source
    Add {
        #[command(subcommand)]
        source: AddSource,
    },
    /// Scan and adopt existing Agent Skill installations
    Adopt {
        /// Folders to scan; defaults to the home folder
        roots: Vec<PathBuf>,
        /// Adopt these exact source folders from the scan
        #[arg(long = "source", conflicts_with = "all")]
        sources: Vec<PathBuf>,
        /// Adopt every unambiguous candidate
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Remove one or more Skills from Kitter and their managed installations
    Remove {
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Install one or more Skills into a project
    Install {
        #[arg(required = true)]
        skills: Vec<String>,
        #[arg(long)]
        project: PathBuf,
        #[arg(long, value_enum, required = true)]
        target: Vec<TargetArg>,
    },
    /// Remove selected Skill installations from a project
    Uninstall {
        skills: Vec<String>,
        /// Remove these exact installation paths
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        project: PathBuf,
        /// Limit removal to these targets; defaults to every installed target
        #[arg(long, value_enum)]
        target: Vec<TargetArg>,
        /// Also remove external links or directly stored Skill folders
        #[arg(long)]
        include_unmanaged: bool,
    },
    /// Inspect direct and effective Skills for a project
    Project {
        path: PathBuf,
        /// Limit effective discovery to one Agent
        #[arg(long, value_enum)]
        agent: Option<AgentArg>,
        /// Select filesystem Skills, plugin Skills, or both
        #[arg(long, value_enum, default_value_t = ProjectViewArg::All)]
        view: ProjectViewArg,
        #[arg(long)]
        json: bool,
    },
    /// Update selected Skills using their recorded sources
    Update {
        skills: Vec<String>,
        /// Update every Skill with a managed source
        #[arg(long, conflicts_with = "skills")]
        all: bool,
    },
    /// Check every Skill for updates
    Check {
        #[arg(long)]
        json: bool,
    },
    /// Manage Skill groups
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Manage Skill tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Show or change where Kitter stores Skills
    Library {
        /// Change the Skill library folder
        #[arg(long, value_name = "PATH")]
        set: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AddSource {
    /// Recursively discover Skills below a local folder
    Local {
        path: PathBuf,
        /// Import only these discovered Skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
    /// Discover Skills from a skills.sh or GitHub source
    Npx {
        repository: String,
        /// Import only these discovered Skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
    /// Discover Skills from a Claude plugin
    Claude {
        plugin: String,
        /// Import only these discovered Skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
}

#[derive(Subcommand)]
enum GroupAction {
    /// List groups and their Skill counts
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a group
    Create { name: String },
    /// Rename a group
    Rename { group: String, name: String },
    /// Delete a group
    Delete {
        group: String,
        /// Delete Skills in the group instead of leaving them ungrouped
        #[arg(long)]
        delete_skills: bool,
    },
    /// Move Skills into a group
    Assign {
        group: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Remove Skills from their groups
    Clear {
        #[arg(required = true)]
        skills: Vec<String>,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// List tags and assignment counts
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a root or child tag
    Create {
        name: String,
        /// Parent tag name or ID for a second-level tag
        #[arg(long, value_name = "TAG")]
        parent: Option<String>,
    },
    /// Rename a tag
    Rename {
        /// Tag name or ID
        tag: String,
        name: String,
    },
    /// Delete a tag and its child tags
    Delete {
        /// Tag name or ID
        tag: String,
    },
    /// Assign a tag to one or more Skills
    Assign {
        /// Tag name or ID
        tag: String,
        #[arg(required = true)]
        items: Vec<String>,
    },
    /// Remove a tag from one or more Skills
    Unassign {
        /// Tag name or ID
        tag: String,
        #[arg(required = true)]
        items: Vec<String>,
    },
    /// Reorder a tag among its siblings
    Move {
        /// Tag name or ID
        tag: String,
        #[arg(
            long,
            value_name = "TAG",
            conflicts_with = "after",
            required_unless_present = "after"
        )]
        before: Option<String>,
        #[arg(
            long,
            value_name = "TAG",
            conflicts_with = "before",
            required_unless_present = "before"
        )]
        after: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum TargetArg {
    Universal,
    Codex,
    Claude,
    Cursor,
    Opencode,
    Pi,
    Grok,
    Antigravity,
    Droid,
    Copilot,
}

impl From<TargetArg> for InstallTarget {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Universal => Self::Universal,
            TargetArg::Codex => Self::Codex,
            TargetArg::Claude => Self::ClaudeCode,
            TargetArg::Cursor => Self::Cursor,
            TargetArg::Opencode => Self::OpenCode,
            TargetArg::Pi => Self::Pi,
            TargetArg::Grok => Self::Grok,
            TargetArg::Antigravity => Self::Antigravity,
            TargetArg::Droid => Self::Droid,
            TargetArg::Copilot => Self::Copilot,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentArg {
    Codex,
    Claude,
    Cursor,
    Opencode,
    Copilot,
    Antigravity,
    Amp,
    Droid,
    Pi,
    Grok,
    Openclaw,
    Hermes,
}

impl From<AgentArg> for AgentKind {
    fn from(value: AgentArg) -> Self {
        match value {
            AgentArg::Codex => Self::Codex,
            AgentArg::Claude => Self::ClaudeCode,
            AgentArg::Cursor => Self::Cursor,
            AgentArg::Opencode => Self::OpenCode,
            AgentArg::Copilot => Self::Copilot,
            AgentArg::Antigravity => Self::Antigravity,
            AgentArg::Amp => Self::Amp,
            AgentArg::Droid => Self::Droid,
            AgentArg::Pi => Self::Pi,
            AgentArg::Grok => Self::Grok,
            AgentArg::Openclaw => Self::OpenClaw,
            AgentArg::Hermes => Self::Hermes,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
enum ProjectViewArg {
    #[default]
    All,
    Skills,
    Plugins,
}

#[derive(Serialize)]
struct SkillOutput {
    id: String,
    name: String,
    description: String,
    origin: kitter::SkillOrigin,
    path: PathBuf,
    group: Option<String>,
    tags: Vec<String>,
    installed_projects: usize,
    manual_only: bool,
    update_available: bool,
}

#[derive(Serialize)]
struct AdoptionOutput {
    name: String,
    source: PathBuf,
    origin: kitter::SkillOrigin,
    references: Vec<kitter::adoption::SkillReference>,
    issue: Option<String>,
    conflict: bool,
    already_managed: bool,
    selectable: bool,
}

#[derive(Serialize)]
struct ProjectOutput {
    path: PathBuf,
    direct_installations: Vec<ProjectSkill>,
    agents: Vec<AgentContextEstimate>,
}

#[derive(Serialize)]
struct GroupOutput {
    id: String,
    name: String,
    skills: usize,
}

#[derive(Serialize)]
struct TagOutput {
    id: String,
    name: String,
    parent: Option<String>,
    assignments: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut library = SkillLibrary::open()?;
    match cli.command {
        Command::List { tag, json } => list_skills(&library, tag.as_deref(), json),
        Command::Show { skill, json } => show_skill(&library, &skill, json),
        Command::Files { skill } => {
            let skill = library.resolve_skill(&skill)?;
            for file in library.files_by_storage(&skill.record.storage_name)? {
                println!("{}", file.display());
            }
            Ok(())
        }
        Command::Read { skill, path } => {
            let skill = library.resolve_skill(&skill)?;
            print!(
                "{}",
                library.read_file_by_storage(&skill.record.storage_name, &path)?
            );
            Ok(())
        }
        Command::Add { source: add } => add_skills(&mut library, add),
        Command::Adopt {
            roots,
            sources,
            all,
            json,
        } => adopt_skills(&mut library, roots, sources, all, json),
        Command::Remove { skills } => remove_skills(&mut library, &skills),
        Command::Install {
            skills,
            project,
            target,
        } => install_skills(&library, &skills, &project, target),
        Command::Uninstall {
            skills,
            paths,
            project,
            target,
            include_unmanaged,
        } => uninstall_skills(
            &library,
            &skills,
            &paths,
            &project,
            target,
            include_unmanaged,
        ),
        Command::Project {
            path,
            agent,
            view,
            json,
        } => inspect_project(&library, &path, agent, view, json),
        Command::Update { skills, all } => update_skills(&mut library, &skills, all),
        Command::Check { json } => {
            let count = source::check_updates(&mut library)?;
            if json {
                println!("{{\"updates\":{count}}}");
            } else {
                println!("{count} Skill(s) can be updated");
            }
            Ok(())
        }
        Command::Group { action } => manage_groups(&mut library, action),
        Command::Tag { action } => manage_tags(&library, action),
        Command::Library { set } => manage_library(&mut library, set),
    }
}

fn list_skills(library: &SkillLibrary, tag: Option<&str>, json: bool) -> Result<()> {
    let (skill_tags, _) = load_tag_states();
    let tag_id = tag.map(|tag| resolve_tag(&skill_tags, tag)).transpose()?;
    let groups = library
        .groups()
        .into_iter()
        .map(|group| (group.id, group.name))
        .collect::<BTreeMap<_, _>>();
    let skills = library
        .list()?
        .into_iter()
        .filter(|skill| {
            tag_id.is_none_or(|tag| skill_tags.matches_filter(&skill.record.storage_name, tag))
        })
        .map(|skill| skill_output(skill, &groups, &skill_tags))
        .collect::<Vec<_>>();
    let duplicate_names = skills.iter().fold(BTreeMap::new(), |mut counts, skill| {
        *counts.entry(skill.name.clone()).or_insert(0usize) += 1;
        counts
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&skills)?);
    } else if skills.is_empty() {
        println!("No Skills found");
    } else {
        for skill in skills {
            let group = skill
                .group
                .as_deref()
                .map(|group| format!("  [{group}]"))
                .unwrap_or_default();
            let tags = if skill.tags.is_empty() {
                String::new()
            } else {
                format!(
                    "  {}",
                    skill
                        .tags
                        .iter()
                        .map(|tag| format!("#{tag}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            let id = if duplicate_names
                .get(&skill.name)
                .copied()
                .unwrap_or_default()
                > 1
            {
                format!("  {}", skill.id)
            } else {
                String::new()
            };
            println!(
                "{:<28} {}{}{}{}",
                skill.name, skill.description, group, tags, id
            );
        }
    }
    Ok(())
}

fn show_skill(library: &SkillLibrary, selector: &str, json: bool) -> Result<()> {
    let skill = library.resolve_skill(selector)?;
    let (skill_tags, _) = load_tag_states();
    let groups = library
        .groups()
        .into_iter()
        .map(|group| (group.id, group.name))
        .collect::<BTreeMap<_, _>>();
    let skill = skill_output(skill, &groups, &skill_tags);
    if json {
        println!("{}", serde_json::to_string_pretty(&skill)?);
    } else {
        println!("{}", skill.name);
        if skill.id != skill.name {
            println!("ID: {}", skill.id);
        }
        println!("{}", skill.description);
        println!("Source: {}", skill.origin.label());
        println!("Path: {}", skill.path.display());
        if let Some(group) = skill.group {
            println!("Group: {group}");
        }
        if !skill.tags.is_empty() {
            println!("Tags: {}", skill.tags.join(", "));
        }
    }
    Ok(())
}

fn skill_output(
    skill: SkillSummary,
    groups: &BTreeMap<String, String>,
    tags: &TagState,
) -> SkillOutput {
    let tag_names = tags
        .assigned_tags(&skill.record.storage_name)
        .into_iter()
        .map(|tag| tag.name.clone())
        .collect();
    SkillOutput {
        id: format!("id:{}", skill.record.storage_name),
        name: skill.record.name.clone(),
        description: skill.record.description.clone(),
        origin: skill.record.origin.clone(),
        path: skill.path,
        group: skill
            .record
            .group_id
            .as_ref()
            .and_then(|id| groups.get(id))
            .cloned(),
        tags: tag_names,
        installed_projects: skill.installed_projects,
        manual_only: skill.manual_only,
        update_available: skill.record.update_available,
    }
}

fn add_skills(library: &mut SkillLibrary, add: AddSource) -> Result<()> {
    let (scan, selected, group) = match add {
        AddSource::Local {
            path,
            skills,
            group,
        } => {
            let path = path
                .canonicalize()
                .with_context(|| format!("找不到来源目录：{}", path.display()))?;
            (source::scan_local(&path)?, skills, group)
        }
        AddSource::Npx {
            repository,
            skills,
            group,
        } => (source::scan_npx(&repository)?, skills, group),
        AddSource::Claude {
            plugin,
            skills,
            group,
        } => (source::scan_claude(&plugin)?, skills, group),
    };
    let available = scan
        .skills()
        .iter()
        .map(|skill| skill.name.clone())
        .collect::<BTreeSet<_>>();
    let selected = if selected.is_empty() {
        available.iter().cloned().collect::<HashSet<_>>()
    } else {
        let missing = selected
            .iter()
            .filter(|skill| !available.contains(*skill))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            bail!("没有从来源中找到：{}", missing.join(", "));
        }
        selected.into_iter().collect()
    };
    let count = scan.import_selected(library, &selected, group.as_deref())?;
    println!("Added {count} Skill(s)");
    Ok(())
}

fn adopt_skills(
    library: &mut SkillLibrary,
    roots: Vec<PathBuf>,
    sources: Vec<PathBuf>,
    all: bool,
    json: bool,
) -> Result<()> {
    let home = dirs::home_dir().context("找不到用户目录")?;
    let roots = if roots.is_empty() {
        vec![home.clone()]
    } else {
        roots
    };
    let managed = library.list()?;
    let scan = adoption::scan_roots(
        &home,
        &roots,
        &library.config.library_dir,
        &managed,
        &AtomicBool::new(false),
    )?;
    let output = scan
        .candidates
        .iter()
        .map(|candidate| AdoptionOutput {
            name: candidate.name.clone(),
            source: candidate.source.clone(),
            origin: candidate.origin.clone(),
            references: candidate.references.clone(),
            issue: candidate.issue.clone(),
            conflict: scan.has_conflict(&candidate.identity()),
            already_managed: candidate.existing_storage.is_some(),
            selectable: scan.selectable_ids().contains(&candidate.id),
        })
        .collect::<Vec<_>>();

    if !all && sources.is_empty() {
        if json {
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if output.is_empty() {
            println!("No existing Skill installations found");
        } else {
            for candidate in output {
                let status = if candidate.issue.is_some() {
                    "unavailable"
                } else if candidate.conflict {
                    "choose-source"
                } else if candidate.already_managed {
                    "managed"
                } else {
                    "ready"
                };
                println!(
                    "{:<24} {:<14} {}",
                    candidate.name,
                    status,
                    candidate.source.display()
                );
            }
            println!("Run with --all or one or more --source paths to adopt");
        }
        return Ok(());
    }

    let mut selected = if all {
        scan.default_selection()
    } else {
        HashSet::new()
    };
    for source in sources {
        let source = source.canonicalize().unwrap_or(source);
        let candidate = scan
            .candidates
            .iter()
            .find(|candidate| candidate.source == source)
            .with_context(|| format!("扫描结果中没有这个来源：{}", source.display()))?;
        if let Some(issue) = &candidate.issue {
            bail!("无法托管 {}：{issue}", candidate.source.display());
        }
        scan.select(&mut selected, &candidate.id);
    }
    if selected.is_empty() {
        bail!("没有可托管的 Skill；冲突来源需要使用 --source 明确选择");
    }

    let mut adopted = 0usize;
    let mut failures = Vec::new();
    for candidate in scan
        .candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.id))
    {
        let result = scan
            .variants(candidate)
            .try_for_each(|variant| variant.verify())
            .and_then(|()| library.adopt(candidate, &scan.references_for(candidate)));
        match result {
            Ok(_) => adopted += 1,
            Err(error) => failures.push(format!("{}：{error:#}", candidate.name)),
        }
    }
    finish_batch("Adopted", "Skill", adopted, failures)
}

fn remove_skills(library: &mut SkillLibrary, selectors: &[String]) -> Result<()> {
    let skills = resolve_skills(library, selectors)?;
    let mut removed = 0usize;
    let mut failures = Vec::new();
    for skill in skills {
        match library.remove_by_storage(&skill.record.storage_name) {
            Ok(()) => removed += 1,
            Err(error) => failures.push(format!("{}：{error:#}", skill.record.name)),
        }
    }
    finish_batch("Removed", "Skill", removed, failures)
}

fn install_skills(
    library: &SkillLibrary,
    selectors: &[String],
    project_path: &Path,
    targets: Vec<TargetArg>,
) -> Result<()> {
    let project_path = canonical_project(project_path)?;
    let skills = resolve_skills(library, selectors)?;
    let targets = targets.into_iter().map(Into::into).collect::<Vec<_>>();
    let mut installed = 0usize;
    let mut failures = Vec::new();
    for skill in skills {
        match project::install_from_path(&project_path, &skill.path, &skill.record.name, &targets) {
            Ok(()) => installed += 1,
            Err(error) => failures.push(format!("{}：{error:#}", skill.record.name)),
        }
    }
    finish_batch("Installed", "Skill", installed, failures)
}

fn uninstall_skills(
    library: &SkillLibrary,
    selectors: &[String],
    paths: &[PathBuf],
    project_path: &Path,
    targets: Vec<TargetArg>,
    include_unmanaged: bool,
) -> Result<()> {
    if selectors.is_empty() && paths.is_empty() {
        bail!("请指定至少一个 Skill 或 --path 安装位置");
    }
    let project_path = canonical_project(project_path)?;
    let installed = project::list(&project_path, &library.config.library_dir)?;
    let library_skills = library.list()?;
    let targets = targets
        .into_iter()
        .map(InstallTarget::from)
        .collect::<HashSet<_>>();
    let mut selected = Vec::<ProjectSkillInstallation>::new();
    for requested in paths {
        let requested = if requested.is_absolute() {
            requested.clone()
        } else {
            project_path.join(requested)
        };
        let requested_key = project::installation_key(&requested);
        let matching = installed
            .iter()
            .flat_map(|skill| skill.installations.iter())
            .filter(|installation| targets.is_empty() || targets.contains(&installation.target))
            .find(|installation| project::installation_key(&installation.path) == requested_key)
            .cloned()
            .with_context(|| format!("这个项目中没有安装位置：{}", requested.display()))?;
        selected.push(matching);
    }
    for selector in selectors {
        let library_matches = matching_library_skills(&library_skills, selector);
        if library_matches.len() > 1 {
            bail!("存在多个名为 {selector} 的 Skill，请使用 list 中的 ID");
        }
        let matching = installed
            .iter()
            .filter(|skill| {
                library_matches
                    .first()
                    .map(|library_skill| skill.name == library_skill.record.name)
                    .unwrap_or_else(|| skill.name == *selector)
            })
            .flat_map(|skill| skill.installations.iter())
            .filter(|installation| targets.is_empty() || targets.contains(&installation.target))
            .filter(|installation| {
                library_matches
                    .first()
                    .is_none_or(|skill| same_file(&installation.path, &skill.path))
            })
            .cloned()
            .collect::<Vec<_>>();
        if matching.is_empty() {
            bail!("这个项目中没有匹配的安装：{selector}");
        }
        selected.extend(matching);
    }
    let unsafe_paths = selected
        .iter()
        .filter(|installation| !installation.managed)
        .map(|installation| installation.path.display().to_string())
        .collect::<Vec<_>>();
    if !unsafe_paths.is_empty() && !include_unmanaged {
        bail!(
            "所选位置包含外部链接或直接保存的文件；确认后使用 --include-unmanaged：{}",
            unsafe_paths.join(", ")
        );
    }
    let report = project::remove_project_skills(&selected);
    finish_batch("Removed", "installation", report.removed, report.failures)
}

fn inspect_project(
    library: &SkillLibrary,
    path: &Path,
    agent: Option<AgentArg>,
    view: ProjectViewArg,
    json: bool,
) -> Result<()> {
    let path = canonical_project(path)?;
    let direct = project::list(&path, &library.config.library_dir)?;
    let selected_agent = agent.map(AgentKind::from);
    let mut estimates = effective_skills::estimate_project(&path)
        .into_iter()
        .filter(|estimate| selected_agent.is_none_or(|agent| estimate.agent == agent))
        .collect::<Vec<_>>();
    for estimate in &mut estimates {
        estimate.skills.retain(|skill| match view {
            ProjectViewArg::All => true,
            ProjectViewArg::Skills => !skill.source.is_plugin(),
            ProjectViewArg::Plugins => skill.source.is_plugin(),
        });
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&ProjectOutput {
                path,
                direct_installations: direct,
                agents: estimates,
            })?
        );
        return Ok(());
    }

    println!("Project: {}", path.display());
    println!("Direct installations: {}", direct.len());
    for skill in &direct {
        let targets = skill
            .installations
            .iter()
            .map(|installation| format!("{:?}", installation.target))
            .collect::<Vec<_>>()
            .join(", ");
        let origin = if skill
            .installations
            .iter()
            .all(|installation| installation.managed)
        {
            "Kitter"
        } else {
            "external"
        };
        println!("  {:<28} {:<12} {targets}", skill.name, origin);
    }
    println!("Agents:");
    for estimate in &estimates {
        println!(
            "  {:<18} {:>3} discovered  {:>3} visible  ~{} tokens",
            estimate.agent.label(),
            estimate.discovered_count,
            estimate.model_visible_count,
            estimate.estimated_tokens
        );
    }
    let effective = group_effective_skills(
        estimates
            .iter()
            .flat_map(|estimate| estimate.skills.iter().map(|skill| (estimate.agent, skill))),
    );
    println!("Effective Skills: {}", effective.len());
    for group in effective {
        let agents = group
            .entries()
            .iter()
            .map(|(agent, _)| agent.label())
            .collect::<BTreeSet<_>>();
        let plugin = group
            .entries()
            .iter()
            .any(|(_, skill)| skill.source.is_plugin());
        let kind = if plugin { "plugin" } else { "skill" };
        println!(
            "  {:<28} {:<7} {}",
            group.name(),
            kind,
            agents.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}

fn update_skills(library: &mut SkillLibrary, selectors: &[String], all: bool) -> Result<()> {
    if selectors.is_empty() && !all {
        bail!("请指定至少一个 Skill，或使用 --all");
    }
    let skills = if all {
        library.list()?
    } else {
        resolve_skills(library, selectors)?
    };
    let mut updated = 0usize;
    let mut failures = Vec::new();
    for skill in skills {
        match source::update_by_storage(library, &skill.record.storage_name) {
            Ok(()) => updated += 1,
            Err(error) => failures.push(format!("{}：{error:#}", skill.record.name)),
        }
    }
    finish_batch("Updated", "Skill", updated, failures)
}

fn manage_groups(library: &mut SkillLibrary, action: GroupAction) -> Result<()> {
    match action {
        GroupAction::List { json } => {
            let skills = library.list()?;
            let groups = library
                .groups()
                .into_iter()
                .map(|group| GroupOutput {
                    skills: skills
                        .iter()
                        .filter(|skill| skill.record.group_id.as_deref() == Some(&group.id))
                        .count(),
                    id: group.id,
                    name: group.name,
                })
                .collect::<Vec<_>>();
            if json {
                println!("{}", serde_json::to_string_pretty(&groups)?);
            } else if groups.is_empty() {
                println!("No groups");
            } else {
                for group in groups {
                    println!("{:<28} {} Skill(s)", group.name, group.skills);
                }
            }
            Ok(())
        }
        GroupAction::Create { name } => {
            library.create_group(&name)?;
            Ok(())
        }
        GroupAction::Rename { group, name } => {
            let group = resolve_group(library, &group)?;
            library.rename_group(&group.id, &name)
        }
        GroupAction::Delete {
            group,
            delete_skills,
        } => {
            let group = resolve_group(library, &group)?;
            library.delete_group(&group.id, delete_skills)?;
            Ok(())
        }
        GroupAction::Assign { group, skills } => {
            let group = resolve_group(library, &group)?;
            let skills = resolve_skills(library, &skills)?;
            for skill in skills {
                library.assign_group_by_storage(&skill.record.storage_name, Some(&group.id))?;
            }
            Ok(())
        }
        GroupAction::Clear { skills } => {
            let skills = resolve_skills(library, &skills)?;
            for skill in skills {
                library.assign_group_by_storage(&skill.record.storage_name, None)?;
            }
            Ok(())
        }
    }
}

fn manage_tags(library: &SkillLibrary, action: TagAction) -> Result<()> {
    let (mut skill_tags, project_tags) = load_tag_states();
    let tags = &mut skill_tags;
    match action {
        TagAction::List { json } => {
            let mut output = Vec::new();
            for root in tags.roots() {
                output.push(TagOutput {
                    id: format!("id:{}", root.id),
                    name: root.name.clone(),
                    parent: None,
                    assignments: tags.count(root.id),
                });
                output.extend(tags.children(root.id).map(|child| TagOutput {
                    id: format!("id:{}", child.id),
                    name: child.name.clone(),
                    parent: Some(root.name.clone()),
                    assignments: tags.count(child.id),
                }));
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if output.is_empty() {
                println!("No tags");
            } else {
                let duplicate_names = output.iter().fold(BTreeMap::new(), |mut counts, tag| {
                    *counts.entry(tag.name.to_lowercase()).or_insert(0usize) += 1;
                    counts
                });
                for tag in output {
                    let indent = if tag.parent.is_some() { "  " } else { "" };
                    let id = if duplicate_names
                        .get(&tag.name.to_lowercase())
                        .copied()
                        .unwrap_or_default()
                        > 1
                    {
                        format!("  {}", tag.id)
                    } else {
                        String::new()
                    };
                    println!(
                        "{indent}#{:<26} {} assignment(s){id}",
                        tag.name, tag.assignments
                    );
                }
            }
            return Ok(());
        }
        TagAction::Create { name, parent } => {
            let parent = parent
                .map(|parent| resolve_tag(tags, &parent))
                .transpose()?;
            tags.add(&name, parent).map_err(anyhow::Error::msg)?;
        }
        TagAction::Rename { tag, name } => {
            let tag = resolve_tag(tags, &tag)?;
            tags.rename(tag, &name).map_err(anyhow::Error::msg)?;
        }
        TagAction::Delete { tag } => {
            let tag = resolve_tag(tags, &tag)?;
            tags.delete(tag);
        }
        TagAction::Assign { tag, items } => {
            let tag = resolve_tag(tags, &tag)?;
            for key in tag_keys(library, &items)? {
                tags.set_assignment(&key, tag, true);
            }
        }
        TagAction::Unassign { tag, items } => {
            let tag = resolve_tag(tags, &tag)?;
            for key in tag_keys(library, &items)? {
                tags.set_assignment(&key, tag, false);
            }
        }
        TagAction::Move { tag, before, after } => {
            let tag = resolve_tag(tags, &tag)?;
            let moved = if let Some(target) = before {
                let target = resolve_tag(tags, &target)?;
                tags.move_before(tag, target)
            } else if let Some(target) = after {
                let target = resolve_tag(tags, &target)?;
                tags.move_after(tag, target)
            } else {
                false
            };
            if !moved {
                bail!("只能在同一级标签之间调整顺序");
            }
        }
    }
    save_tag_states(&skill_tags, &project_tags)
}

fn manage_library(library: &mut SkillLibrary, set: Option<PathBuf>) -> Result<()> {
    if let Some(path) = set {
        if !path.is_absolute() {
            bail!("请使用绝对路径");
        }
        std::fs::create_dir_all(&path)?;
        library.config.library_dir = path;
        library.save()?;
    }
    println!("{}", library.config.library_dir.display());
    Ok(())
}

fn resolve_skills(library: &SkillLibrary, selectors: &[String]) -> Result<Vec<SkillSummary>> {
    let mut seen = HashSet::new();
    let mut skills = Vec::new();
    for selector in selectors {
        let skill = library.resolve_skill(selector)?;
        if seen.insert(skill.record.storage_name.clone()) {
            skills.push(skill);
        }
    }
    Ok(skills)
}

fn matching_library_skills<'a>(
    skills: &'a [SkillSummary],
    selector: &str,
) -> Vec<&'a SkillSummary> {
    if let Some(storage_name) = selector.strip_prefix("id:") {
        return skills
            .iter()
            .filter(|skill| skill.record.storage_name == storage_name)
            .collect();
    }
    skills
        .iter()
        .filter(|skill| skill.record.name == selector)
        .collect()
}

fn resolve_group(library: &SkillLibrary, selector: &str) -> Result<SkillGroup> {
    library
        .groups()
        .into_iter()
        .find(|group| group.id == selector || group.name.eq_ignore_ascii_case(selector))
        .with_context(|| format!("没有找到分组：{selector}"))
}

fn resolve_tag(tags: &TagState, selector: &str) -> Result<TagId> {
    let selector = selector.trim().trim_start_matches('#').trim();
    if let Some(id) = selector.strip_prefix("id:") {
        let id = id
            .parse::<TagId>()
            .with_context(|| format!("无效的标签 ID：{selector}"))?;
        return tags
            .tag(id)
            .map(|tag| tag.id)
            .with_context(|| format!("没有找到标签：{selector}"));
    }
    let matches = tags
        .tags()
        .iter()
        .filter(|tag| tag.name.eq_ignore_ascii_case(selector))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [tag] => Ok(tag.id),
        [] => bail!("没有找到标签：{selector}"),
        _ => bail!("存在多个名为 {selector} 的标签，请使用 list 中显示的 ID"),
    }
}

fn tag_keys(library: &SkillLibrary, items: &[String]) -> Result<Vec<String>> {
    Ok(resolve_skills(library, items)?
        .into_iter()
        .map(|skill| skill.record.storage_name)
        .collect())
}

fn canonical_project(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        bail!("项目文件夹不存在：{}", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("无法读取项目：{}", path.display()))
}

fn same_file(left: &Path, right: &Path) -> bool {
    left.canonicalize().ok() == right.canonicalize().ok()
}

fn finish_batch(action: &str, item: &str, succeeded: usize, failures: Vec<String>) -> Result<()> {
    if failures.is_empty() {
        println!("{action} {succeeded} {item}(s)");
        return Ok(());
    }
    if succeeded > 0 {
        eprintln!("{action} {succeeded} {item}(s)");
    }
    bail!(
        "{} operation(s) failed：{}",
        failures.len(),
        failures.join("；")
    )
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};
    use kitter::tags::TagState;

    use super::{Cli, Command, ProjectViewArg, TagAction, resolve_tag};

    #[test]
    fn command_tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_batch_install() {
        let cli = Cli::try_parse_from([
            "kitter",
            "install",
            "alpha",
            "beta",
            "--project",
            "/tmp/project",
            "--target",
            "universal",
            "--target",
            "codex",
        ])
        .unwrap();
        let Command::Install { skills, target, .. } = cli.command else {
            panic!("expected install command");
        };
        assert_eq!(skills, ["alpha", "beta"]);
        assert_eq!(target.len(), 2);
    }

    #[test]
    fn parses_project_plugin_view() {
        let cli = Cli::try_parse_from([
            "kitter",
            "project",
            "/tmp/project",
            "--agent",
            "codex",
            "--view",
            "plugins",
        ])
        .unwrap();
        let Command::Project { view, .. } = cli.command else {
            panic!("expected project command");
        };
        assert!(matches!(view, ProjectViewArg::Plugins));
    }

    #[test]
    fn parses_skill_tag_assignment() {
        let cli = Cli::try_parse_from(["kitter", "tag", "assign", "client", "skill-a", "skill-b"])
            .unwrap();
        let Command::Tag {
            action: TagAction::Assign { items, .. },
        } = cli.command
        else {
            panic!("expected Skill tag assignment");
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn client_settings_are_not_cli_commands() {
        assert!(Cli::try_parse_from(["kitter", "config", "set-theme", "dark"]).is_err());
        let cli = Cli::try_parse_from(["kitter", "library", "--set", "/tmp/skills"])
            .expect("Skill library location should remain configurable");
        let Command::Library { set } = cli.command else {
            panic!("expected library command");
        };
        assert_eq!(set.unwrap(), std::path::PathBuf::from("/tmp/skills"));
    }

    #[test]
    fn tags_use_names_and_only_require_ids_for_duplicates() {
        let mut tags = TagState::default();
        let work = tags.add("work", None).unwrap();
        let personal = tags.add("personal", None).unwrap();
        let work_client = tags.add("client", Some(work)).unwrap();
        tags.add("client", Some(personal)).unwrap();

        assert_eq!(resolve_tag(&tags, "work").unwrap(), work);
        assert!(resolve_tag(&tags, "client").is_err());
        assert_eq!(
            resolve_tag(&tags, &format!("id:{work_client}")).unwrap(),
            work_client
        );
    }
}
