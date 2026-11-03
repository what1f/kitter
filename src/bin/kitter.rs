// Reuse the library backend; do not embed a second translation catalogue.
use kitter::_rust_i18n_try_translate;
use rust_i18n::_tr as t;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use kitter::{
    InstallTarget, ProjectSkill, ProjectSkillInstallation, SkillGroup, SkillLibrary, SkillSummary,
    adoption,
    agent_plugins::{
        AgentRegistry, BUILTIN_PLUGINS, PluginOrigin,
        manage::{self, CandidateStatus, SourceRecord},
        plugin_dir,
    },
    effective_skills::{self, AgentContextEstimate, AgentId, group_effective_skills},
    preset::{
        AgentsMdState, FileConflictDecision, FileDecisions, FileState, FileStatus, Preset,
        PresetStore, SkillFailure,
    },
    project, source,
    tags::{TagId, TagState, load_tag_states, save_tag_states},
    themes::{self, ThemeSource},
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
    /// List agents and plugin errors
    Agents {
        #[arg(long, global = true)]
        json: bool,
        #[command(subcommand)]
        action: Option<AgentsAction>,
    },
    /// List skills saved in Kitter
    List {
        /// Only show skills carrying this tag
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show one skill by name or ID
    Show {
        skill: String,
        #[arg(long)]
        json: bool,
    },
    #[command(about = t!("cli.describe_help").to_string())]
    Describe {
        skill: String,
        #[arg(required_unless_present = "reset", conflicts_with = "reset")]
        text: Option<String>,
        #[arg(long, help = t!("cli.describe_reset_help").to_string())]
        reset: bool,
    },
    /// List files in one skill
    Files { skill: String },
    /// Read one file from a skill
    Read { skill: String, path: PathBuf },
    /// Add skills from a source
    Add {
        #[command(subcommand)]
        source: AddSource,
    },
    /// Scan and adopt existing agent skill installations
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
    /// Remove one or more skills from Kitter and their managed installations
    Remove {
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Install one or more skills into a project
    Install {
        #[arg(required = true)]
        skills: Vec<String>,
        #[arg(long)]
        project: PathBuf,
        #[arg(long, required = true)]
        target: Vec<String>,
        /// Confirm overwriting existing skills
        #[arg(short, long)]
        yes: bool,
    },
    /// Remove selected skill installations from a project
    Uninstall {
        skills: Vec<String>,
        /// Remove these exact installation paths
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        project: PathBuf,
        /// Limit removal to these targets; defaults to every installed target
        #[arg(long)]
        target: Vec<String>,
        /// Also remove external links or directly stored skill folders
        #[arg(long)]
        include_unmanaged: bool,
    },
    /// Inspect direct and effective skills for a project
    Project {
        path: PathBuf,
        /// Limit effective discovery to one agent
        #[arg(long)]
        agent: Option<String>,
        /// Select filesystem skills, plugin skills, or both
        #[arg(long, value_enum, default_value_t = ProjectViewArg::All)]
        view: ProjectViewArg,
        #[arg(long)]
        json: bool,
    },
    /// Update selected skills using their recorded sources
    Update {
        skills: Vec<String>,
        /// Update every skill with a managed source
        #[arg(long, conflicts_with = "skills")]
        all: bool,
    },
    /// Check every skill for updates
    Check {
        #[arg(long)]
        json: bool,
    },
    /// Manage skill groups
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Manage skill tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Manage presets: bundles of skills and an AGENTS.md installed together
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
    /// List, check and pick desktop themes
    Theme {
        #[arg(long, global = true)]
        json: bool,
        #[command(subcommand)]
        action: Option<ThemeAction>,
    },
    /// Show where Kitter stores skills
    Library,
    /// Show or change the folder that holds all Kitter data
    Data {
        /// Move the data into an empty folder, or switch to a folder that already has Kitter data
        #[arg(long, value_name = "PATH")]
        set: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ThemeAction {
    /// List themes and user themes that fail to load
    List,
    /// Print every token of a theme as a theme.json
    Show { id: String },
    /// Report ignored keys, invalid values and unknown icons in a theme
    Check {
        /// Theme id, or path to a theme file or theme package folder
        theme: String,
    },
    /// Use a theme in the desktop app
    Use { id: String },
    /// Create a theme package holding every token of another theme
    Create {
        id: String,
        /// Theme to start from
        #[arg(long, default_value = themes::STANDARD)]
        from: String,
        /// Name shown in the theme menu
        #[arg(long)]
        name: Option<String>,
    },
    /// Copy a theme file or theme package folder into the themes folder
    Add {
        path: PathBuf,
        /// Theme id; defaults to the file or folder name
        #[arg(long)]
        id: Option<String>,
        /// Replace an existing theme with the same id
        #[arg(long)]
        force: bool,
    },
    /// Delete a theme file or theme package
    Remove { id: String },
    /// List the icons a theme package can replace
    Icons {
        /// Mark the icons this theme replaces
        #[arg(long)]
        theme: Option<String>,
    },
}

#[derive(Subcommand)]
enum AgentsAction {
    /// Add agents from a GitHub URL or local path
    Add {
        source: String,
        #[arg(long)]
        only: Vec<String>,
    },
    /// Remove a managed agent
    Remove { id: String },
    /// Update managed agents
    Update { id: Option<String> },
    /// Enable an agent
    Enable { id: String },
    /// Disable an agent
    Disable { id: String },
    /// Move an agent before or after another agent
    Move {
        id: String,
        #[arg(long, required_unless_present = "after", conflicts_with = "after")]
        before: Option<String>,
        #[arg(long, required_unless_present = "before")]
        after: Option<String>,
    },
    /// Check plugins and inspect effective skills
    Check {
        file: Option<PathBuf>,
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AddSource {
    /// Recursively discover skills below a local folder
    Local {
        path: PathBuf,
        #[arg(long, help = t!("cli.copy_help").to_string())]
        copy: bool,
        /// Import only these discovered skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
    /// Discover skills from a skills.sh or GitHub source
    Npx {
        repository: String,
        /// Import only these discovered skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
    /// Discover skills from a Claude plugin
    Claude {
        plugin: String,
        /// Import only these discovered skill names; defaults to all
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long)]
        group: Option<String>,
    },
}

#[derive(Subcommand)]
enum GroupAction {
    /// List groups and their skill counts
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
        /// Delete skills in the group instead of leaving them ungrouped
        #[arg(long)]
        delete_skills: bool,
    },
    /// Move skills into a group
    Assign {
        group: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Remove skills from their groups
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
    /// Assign a tag to one or more skills
    Assign {
        /// Tag name or ID
        tag: String,
        #[arg(required = true)]
        items: Vec<String>,
    },
    /// Remove a tag from one or more skills
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

#[derive(Subcommand)]
enum PresetAction {
    /// List presets
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show a preset's skills, AGENTS.md and projects
    Show {
        preset: String,
        #[arg(long)]
        json: bool,
    },
    /// Create an empty preset
    Create { name: String },
    /// Rename a preset
    Rename { preset: String, name: String },
    /// Delete a preset and remove it from every project it was installed in
    Delete { preset: String },
    /// Add skills to a preset
    Add {
        preset: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Remove skills from a preset
    Remove {
        preset: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// Print a preset's AGENTS.md, or replace it
    AgentsMd {
        preset: String,
        /// Read the new content from this file; use - for stdin
        #[arg(long, value_name = "FILE", conflicts_with = "clear")]
        set: Option<PathBuf>,
        /// Remove the preset's AGENTS.md
        #[arg(long)]
        clear: bool,
    },
    #[command(about = t!("cli.preset_files_help").to_string())]
    File {
        #[command(subcommand)]
        action: PresetFileAction,
    },
    /// Install a preset into a project
    #[command(alias = "apply")]
    Install {
        preset: String,
        #[arg(long)]
        project: PathBuf,
        #[arg(long, required = true)]
        target: Vec<String>,
        /// Confirm overwriting existing skills
        #[arg(short, long)]
        yes: bool,
        #[arg(long, conflicts_with = "skip_conflicts", help = t!("cli.overwrite_conflicts_help").to_string())]
        overwrite_conflicts: bool,
        #[arg(long, help = t!("cli.skip_conflicts_help").to_string())]
        skip_conflicts: bool,
    },
    /// Sync projects with the latest preset content
    Sync {
        /// Only sync this preset; defaults to every preset
        preset: Option<String>,
        /// Only sync this project; defaults to every project
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long, help = t!("cli.preset_force_help").to_string())]
        force: bool,
        #[arg(long, conflicts_with = "skip_conflicts", help = t!("cli.overwrite_conflicts_help").to_string())]
        overwrite_conflicts: bool,
        #[arg(long, help = t!("cli.skip_conflicts_help").to_string())]
        skip_conflicts: bool,
    },
    /// Remove a preset from a project
    Uninstall {
        preset: String,
        #[arg(long)]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum PresetFileAction {
    #[command(about = t!("cli.preset_file_list_help").to_string())]
    List {
        preset: String,
        #[arg(long)]
        json: bool,
    },
    #[command(about = t!("cli.preset_file_read_help").to_string())]
    Read { preset: String, path: PathBuf },
    #[command(about = t!("cli.preset_file_write_help").to_string())]
    Write {
        preset: String,
        path: PathBuf,
        source: PathBuf,
    },
    #[command(about = t!("cli.preset_file_rename_help").to_string())]
    Rename {
        preset: String,
        from: PathBuf,
        to: PathBuf,
    },
    #[command(about = t!("cli.preset_file_delete_help").to_string())]
    Delete { preset: String, path: PathBuf },
}

fn parse_target(input: &str) -> std::result::Result<InstallTarget, String> {
    let registry = kitter::agent_plugins::AgentRegistry::global();
    if input == "universal" {
        return Ok(InstallTarget::universal());
    }
    if let Some(id) = registry.resolve(input)
        && registry
            .agent(id.as_str())
            .is_some_and(|agent| agent.install.is_some())
    {
        return Ok(InstallTarget::from(id.as_str()));
    }
    Err(t!(
        "errors.target_invalid",
        target = input,
        values = registry
            .install_targets()
            .iter()
            .map(|target| target.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
    .to_string())
}

fn parse_agent(input: &str) -> std::result::Result<AgentId, String> {
    let registry = kitter::agent_plugins::AgentRegistry::global();
    registry.resolve(input).ok_or_else(|| {
        t!(
            "errors.agent_invalid",
            agent = input,
            values = registry
                .agents()
                .iter()
                .map(|agent| agent.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .to_string()
    })
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
    original_description: String,
    custom_description: Option<String>,
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
struct PresetOutput {
    id: String,
    name: String,
    skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents_md: Option<String>,
    files: Vec<PathBuf>,
    projects: Vec<PresetProjectOutput>,
}

#[derive(Serialize)]
struct PresetProjectOutput {
    path: PathBuf,
    targets: Vec<InstallTarget>,
    needs_sync: bool,
    agents_md_modified: bool,
    files: Vec<FileStatus>,
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
        eprintln!(
            "{}",
            t!("cli.error", error = kitter::i18n::error_diagnostic(&error))
        );
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    kitter::i18n::apply_language(kitter::config::Language::System);
    kitter::i18n::apply_language(kitter::AppConfig::load()?.language);
    let cli = Cli::parse();
    if let Command::Agents { json, action } = cli.command {
        return inspect_agents(action, json);
    }
    if let Command::Theme { json, action } = cli.command {
        return manage_themes(action.unwrap_or(ThemeAction::List), json);
    }
    if let Command::Data { set } = cli.command {
        return manage_data(set);
    }
    if !AgentRegistry::global().diagnostics().is_empty() {
        eprintln!("{}", t!("cli.plugin_warning"));
    }
    let mut library = SkillLibrary::open()?;
    match cli.command {
        Command::Agents { .. } | Command::Theme { .. } | Command::Data { .. } => unreachable!(),
        Command::List { tag, json } => list_skills(&library, tag.as_deref(), json),
        Command::Show { skill, json } => show_skill(&library, &skill, json),
        Command::Describe { skill, text, .. } => {
            library.set_description(&skill, text.as_deref().unwrap_or_default())?;
            println!("{}", t!("cli.description_saved"));
            Ok(())
        }
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
            yes,
        } => install_skills(&library, &skills, &project, target, yes),
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
                println!(
                    "{}",
                    kitter::i18n::tn("cli.update_count", count, &[("count", (count).to_string())])
                );
            }
            Ok(())
        }
        Command::Group { action } => manage_groups(&mut library, action),
        Command::Tag { action } => manage_tags(&library, action),
        Command::Preset { action } => manage_presets(&library, action),
        Command::Library => {
            println!("{}", library.config.library_dir.display());
            Ok(())
        }
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
        println!("{}", t!("cli.no_skills_found").to_string());
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
            println!("{}", t!("cli.id", id = skill.id).to_string());
        }
        println!("{}", skill.description);
        println!(
            "{}",
            t!("cli.source", source = skill.origin.label()).to_string()
        );
        println!(
            "{}",
            t!("cli.path", path = skill.path.display()).to_string()
        );
        if let Some(group) = skill.group {
            println!("{}", t!("cli.group", group = group).to_string());
        }
        if !skill.tags.is_empty() {
            println!(
                "{}",
                t!("cli.tags", tags = skill.tags.join(", ")).to_string()
            );
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
        original_description: skill.original_description,
        custom_description: skill.record.custom_description.clone(),
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
    let mut install_mode = kitter::LocalInstallMode::Link;
    let (scan, selected, group) = match add {
        AddSource::Local {
            path,
            copy,
            skills,
            group,
        } => {
            if copy {
                install_mode = kitter::LocalInstallMode::Copy;
            }
            let path = path.canonicalize().with_context(|| {
                t!("errors.source_folder_missing", path = path.display()).to_string()
            })?;
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
            bail!(t!("errors.not_found_in_source", names = missing.join(", ")).to_string());
        }
        selected.into_iter().collect()
    };
    let summary =
        scan.import_selected_with_mode(library, &selected, group.as_deref(), install_mode)?;
    println!(
        "{}",
        t!(
            "cli.import_summary",
            added = kitter::i18n::tn(
                "common.skill_count",
                summary.added,
                &[("count", (summary.added).to_string())]
            ),
            skipped = kitter::i18n::tn(
                "common.skill_count",
                summary.skipped,
                &[("count", (summary.skipped).to_string())]
            )
        )
        .to_string()
    );
    Ok(())
}

fn adopt_skills(
    library: &mut SkillLibrary,
    roots: Vec<PathBuf>,
    sources: Vec<PathBuf>,
    all: bool,
    json: bool,
) -> Result<()> {
    let home = dirs::home_dir().context(kitter::user_error!("errors.home_missing"))?;
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
            issue: candidate
                .issue
                .as_ref()
                .map(|_| t!("errors.sources_mismatch", locale = "zh-CN").to_string()),
            conflict: scan.has_conflict(&candidate.identity()),
            already_managed: candidate.existing_storage.is_some(),
            selectable: scan.selectable_ids().contains(&candidate.id),
        })
        .collect::<Vec<_>>();

    if !all && sources.is_empty() {
        if json {
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if output.is_empty() {
            println!("{}", t!("cli.no_installations").to_string());
        } else {
            for candidate in output {
                let status = if candidate.issue.is_some() {
                    t!("cli.unavailable").to_string()
                } else if candidate.conflict {
                    t!("cli.choose_source").to_string()
                } else if candidate.already_managed {
                    t!("cli.managed").to_string()
                } else {
                    t!("cli.ready").to_string()
                };
                println!(
                    "{:<24} {:<14} {}",
                    candidate.name,
                    status,
                    candidate.source.display()
                );
            }
            println!("{}", t!("cli.adopt_hint").to_string());
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
            .with_context(|| {
                t!("errors.scan_source_missing", path = source.display()).to_string()
            })?;
        if let Some(issue) = &candidate.issue {
            bail!(
                t!(
                    "errors.cannot_adopt",
                    path = candidate.source.display(),
                    issue = issue
                )
                .to_string()
            );
        }
        scan.select(&mut selected, &candidate.id);
    }
    if selected.is_empty() {
        bail!(t!("errors.adopt_source_required").to_string());
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
            Err(error) => failures.push(
                t!(
                    "errors.item_detail",
                    item = candidate.name,
                    error = kitter::i18n::error_diagnostic(&error)
                )
                .to_string(),
            ),
        }
    }
    finish_batch("common.adopted_skills", adopted, failures)
}

fn remove_skills(library: &mut SkillLibrary, selectors: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    let mut skills = Vec::new();
    for selector in selectors {
        let record = library.removal_record(selector)?;
        if seen.insert(record.storage_name.clone()) {
            skills.push(record);
        }
    }
    let mut removed = 0usize;
    let mut failures = Vec::new();
    for skill in skills {
        match library.remove_by_storage(&skill.storage_name) {
            Ok(()) => removed += 1,
            Err(error) => failures.push(
                t!(
                    "errors.item_detail",
                    item = skill.name,
                    error = kitter::i18n::error_diagnostic(&error)
                )
                .to_string(),
            ),
        }
    }
    finish_batch("cli.removed_skills", removed, failures)
}

fn confirm_install(plan: &project::InstallPlan, yes: bool) -> Result<bool> {
    let names = plan
        .conflicts()
        .map(|conflict| conflict.name.as_str())
        .collect::<BTreeSet<_>>();
    if names.is_empty() || yes {
        return Ok(true);
    }
    eprintln!("{}", t!("install.overwrite_warning"));
    for name in names {
        eprintln!("  {name}");
    }
    if !std::io::stdin().is_terminal() {
        bail!(t!("install.overwrite_requires_yes").to_string());
    }
    eprint!("{} ", t!("install.confirm_overwrite"));
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn install_skills(
    library: &SkillLibrary,
    selectors: &[String],
    project_path: &Path,
    targets: Vec<String>,
    yes: bool,
) -> Result<()> {
    let project_path = canonical_project(project_path)?;
    let skills = resolve_skills(library, selectors)?;
    let targets = targets
        .iter()
        .map(|target| parse_target(target))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(anyhow::Error::msg)?;
    let mut plan = project::InstallPlan::default();
    for skill in &skills {
        plan.add(&project_path, &skill.path, &skill.record.name, &targets)?;
    }
    if !confirm_install(&plan, yes)? {
        return Ok(());
    }
    plan.execute()?;
    finish_batch("common.installed_skills", skills.len(), Vec::new())
}

fn uninstall_skills(
    library: &SkillLibrary,
    selectors: &[String],
    paths: &[PathBuf],
    project_path: &Path,
    targets: Vec<String>,
    include_unmanaged: bool,
) -> Result<()> {
    if selectors.is_empty() && paths.is_empty() {
        bail!(t!("errors.removal_selection_required").to_string());
    }
    let project_path = canonical_project(project_path)?;
    let installed = project::list(&project_path, &library.config.library_dir)?;
    let library_skills = library.list()?;
    let targets = targets
        .into_iter()
        .map(|target| parse_target(&target))
        .collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(anyhow::Error::msg)?;
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
            .with_context(|| {
                t!("errors.installation_not_found", path = requested.display()).to_string()
            })?;
        selected.push(matching);
    }
    for selector in selectors {
        let library_matches = matching_library_skills(&library_skills, selector);
        if library_matches.len() > 1 {
            bail!(t!("errors.cli_skill_ambiguous", name = selector).to_string());
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
            bail!(t!("errors.installation_unmatched", name = selector).to_string());
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
            t!(
                "errors.unmanaged_confirmation",
                paths = unsafe_paths.join(", ")
            )
            .to_string()
        );
    }
    let report = project::remove_project_skills(&selected);
    finish_batch("cli.removed_installations", report.removed, report.failures)
}

#[derive(Serialize)]
struct AgentOutput<'a> {
    id: &'a AgentId,
    name: &'a str,
    origin: &'static str,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a SourceRecord>,
}

#[derive(Serialize)]
struct DiagnosticOutput<'a> {
    origin: &'static str,
    id: &'a Option<String>,
    message: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentsOutput<'a> {
    agents: Vec<AgentOutput<'a>>,
    plugin_dir: PathBuf,
    diagnostics: Vec<DiagnosticOutput<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimates: Option<&'a [AgentContextEstimate]>,
}

fn origin_name(origin: &PluginOrigin) -> &'static str {
    match origin {
        PluginOrigin::Builtin => "builtin",
        PluginOrigin::User(_) => "user",
        PluginOrigin::Override(_) => "override",
    }
}

fn inspect_agents(action: Option<AgentsAction>, json: bool) -> Result<()> {
    let selected_registry;
    let (registry, project) = match action {
        Some(AgentsAction::Check { file, project }) => {
            let registry = if let Some(file) = file {
                selected_registry = AgentRegistry::load(BUILTIN_PLUGINS, &[file]);
                &selected_registry
            } else {
                AgentRegistry::global()
            };
            let path = project.map(Ok).unwrap_or_else(std::env::current_dir)?;
            (registry, Some(canonical_project(&path)?))
        }
        None => (AgentRegistry::global(), None),
        Some(action) => return manage_agents(action),
    };
    let (estimates, estimate_diagnostics) = project
        .as_ref()
        .map(|path| effective_skills::check_project(registry, path))
        .unwrap_or_default();
    let diagnostics = registry
        .diagnostics()
        .iter()
        .chain(&estimate_diagnostics)
        .collect::<Vec<_>>();
    let settings = manage::settings();
    let listed = if project.is_some() {
        registry.agents()
    } else {
        registry.all_agents()
    };
    if json {
        let output = AgentsOutput {
            agents: listed
                .iter()
                .map(|agent| AgentOutput {
                    id: &agent.id,
                    name: &agent.name,
                    origin: origin_name(&agent.origin),
                    enabled: registry.is_enabled(agent.id.as_str()),
                    source: settings.sources.get(agent.id.as_str()),
                })
                .collect(),
            plugin_dir: plugin_dir(),
            diagnostics: diagnostics
                .iter()
                .map(|diagnostic| DiagnosticOutput {
                    origin: origin_name(&diagnostic.origin),
                    id: &diagnostic.id,
                    message: &diagnostic.message,
                })
                .collect(),
            estimates: project.as_ref().map(|_| estimates.as_slice()),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{}",
            t!("cli.plugin_directory", path = plugin_dir().display())
        );
        for agent in listed {
            let origin = t!(format!("cli.origin_{}", origin_name(&agent.origin))).to_string();
            let disabled = if registry.is_enabled(agent.id.as_str()) {
                String::new()
            } else {
                format!("  {}", t!("agent_manage.disabled"))
            };
            println!(
                "  {:<18} {:<20} {}{}",
                agent.id.as_str(),
                agent.name,
                origin,
                disabled
            );
        }
        for diagnostic in diagnostics {
            let origin = match &diagnostic.origin {
                PluginOrigin::Builtin => t!("cli.origin_builtin").to_string(),
                PluginOrigin::User(path) | PluginOrigin::Override(path) => {
                    path.display().to_string()
                }
            };
            println!(
                "{}",
                t!(
                    "cli.plugin_diagnostic",
                    origin = origin,
                    id = diagnostic.id.as_deref().unwrap_or(""),
                    message = &diagnostic.message
                )
            );
        }
        if let Some(path) = project {
            println!("{}", t!("cli.project", path = path.display()));
            print_effective(registry, &estimates);
        }
    }
    Ok(())
}

fn manage_agents(action: AgentsAction) -> Result<()> {
    match action {
        AgentsAction::Add { source, only } => {
            let fetched = manage::fetch(manage::parse_source(&source)?)?;
            for candidate in &fetched.candidates {
                let reason = match &candidate.status {
                    CandidateStatus::AlreadyAdded => {
                        Some(t!("agent_manage.already_added").to_string())
                    }
                    CandidateStatus::Invalid(error) => Some(error.clone()),
                    _ => None,
                };
                if let Some(reason) = reason {
                    eprintln!(
                        "{}",
                        t!("agent_manage.skipped", id = &candidate.id, reason = reason)
                    );
                }
            }
            let ids = if only.is_empty() {
                fetched
                    .candidates
                    .iter()
                    .filter(|candidate| {
                        matches!(
                            candidate.status,
                            CandidateStatus::Ready | CandidateStatus::Override
                        )
                    })
                    .map(|candidate| candidate.id.clone())
                    .collect()
            } else {
                only
            };
            if ids.is_empty() {
                bail!(t!("agent_manage.none_added"));
            }
            manage::add(&fetched, &ids)?;
            for id in ids {
                println!("{}", t!("agent_manage.added", id = id));
            }
        }
        AgentsAction::Remove { id } => {
            manage::remove(&id)?;
            println!("{}", t!("agent_manage.removed", id = id));
        }
        AgentsAction::Update { id } => {
            let mut check_failed = false;
            let ids =
                if let Some(id) = id {
                    vec![id]
                } else {
                    let (mut ids, failures) = manage::check_updates_with_failures()?;
                    check_failed = !failures.is_empty();
                    for (repo, error) in failures {
                        eprintln!(
                            "{}",
                            t!("agent_manage.update_failed", repo = repo, error = error)
                        );
                    }
                    ids.extend(manage::settings().sources.into_iter().filter_map(
                        |(id, source)| matches!(source, SourceRecord::Local { .. }).then_some(id),
                    ));
                    ids
                };
            let mut updated = false;
            for id in ids {
                if manage::refresh(&id)? {
                    updated = true;
                    println!("{}", t!("agent_manage.updated", id = id));
                }
            }
            if !updated && !check_failed {
                println!("{}", t!("agent_manage.up_to_date"));
            }
        }
        AgentsAction::Enable { id } => {
            manage::set_enabled(&id, true)?;
            println!("{}", t!("agent_manage.enabled_agent", id = id));
        }
        AgentsAction::Disable { id } => {
            manage::set_enabled(&id, false)?;
            println!("{}", t!("agent_manage.disabled_agent", id = id));
        }
        AgentsAction::Move { id, before, after } => {
            let is_after = after.is_some();
            manage::move_agent(&id, before.as_ref().or(after.as_ref()).unwrap(), is_after)?;
            println!("{}", t!("agent_manage.moved", id = id));
        }
        AgentsAction::Check { .. } => unreachable!(),
    }
    Ok(())
}

fn inspect_project(
    library: &SkillLibrary,
    path: &Path,
    agent: Option<String>,
    view: ProjectViewArg,
    json: bool,
) -> Result<()> {
    let path = canonical_project(path)?;
    let direct = project::list(&path, &library.config.library_dir)?;
    let selected_agent = agent
        .as_deref()
        .map(parse_agent)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let view = match view {
        ProjectViewArg::All => effective_skills::SkillView::All,
        ProjectViewArg::Skills => effective_skills::SkillView::Skills,
        ProjectViewArg::Plugins => effective_skills::SkillView::Plugins,
    };
    let estimates = effective_skills::estimate_project_view(&path, view)
        .into_iter()
        .filter(|estimate| {
            selected_agent
                .as_ref()
                .is_none_or(|agent| &estimate.agent == agent)
        })
        .collect::<Vec<_>>();
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

    println!("{}", t!("cli.project", path = path.display()).to_string());
    println!(
        "{}",
        t!("cli.direct_installations", count = direct.len()).to_string()
    );
    for skill in &direct {
        let targets = skill
            .installations
            .iter()
            .map(|installation| installation.target.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let origin = if skill
            .installations
            .iter()
            .all(|installation| installation.managed)
        {
            "Kitter".to_string()
        } else {
            t!("cli.external").to_string()
        };
        println!("  {:<28} {:<12} {targets}", skill.name, origin);
    }
    print_effective(AgentRegistry::global(), &estimates);
    Ok(())
}

fn print_effective(registry: &AgentRegistry, estimates: &[AgentContextEstimate]) {
    println!("{}", t!("cli.agents").to_string());
    for estimate in estimates {
        println!(
            "{}",
            t!(
                "cli.agent_summary",
                agent = format!(
                    "{:<18}",
                    registry
                        .agent(estimate.agent.as_str())
                        .unwrap()
                        .name
                        .as_str()
                ),
                discovered = format!("{:>3}", estimate.discovered_count),
                visible = format!("{:>3}", estimate.model_visible_count),
                tokens = estimate.estimated_tokens
            )
            .to_string()
        );
    }
    let effective = group_effective_skills(estimates.iter().flat_map(|estimate| {
        estimate
            .skills
            .iter()
            .map(|skill| (estimate.agent.clone(), skill))
    }));
    println!(
        "{}",
        t!("cli.effective_skills", count = effective.len()).to_string()
    );
    for group in effective {
        let agents = group
            .entries()
            .iter()
            .map(|(agent, _)| registry.agent(agent.as_str()).unwrap().name.as_str())
            .collect::<BTreeSet<_>>();
        let plugin = group
            .entries()
            .iter()
            .any(|(_, skill)| skill.source.is_plugin());
        let kind = if plugin {
            t!("cli.plugin").to_string()
        } else {
            t!("cli.skill").to_string()
        };
        println!(
            "  {:<28} {:<7} {}",
            group.name(),
            kind,
            agents.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
}

fn update_skills(library: &mut SkillLibrary, selectors: &[String], all: bool) -> Result<()> {
    if selectors.is_empty() && !all {
        bail!(t!("errors.update_selection_required").to_string());
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
            Err(error) => failures.push(
                t!(
                    "errors.item_detail",
                    item = skill.record.name,
                    error = kitter::i18n::error_diagnostic(&error)
                )
                .to_string(),
            ),
        }
    }
    finish_batch("cli.updated_skills", updated, failures)
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
                println!("{}", t!("cli.no_groups").to_string());
            } else {
                for group in groups {
                    println!(
                        "{:<28} {}",
                        group.name,
                        kitter::i18n::tn(
                            "common.skill_count",
                            group.skills,
                            &[("count", (group.skills).to_string())]
                        )
                    );
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
                println!("{}", t!("cli.no_tags").to_string());
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
                        "{indent}#{:<26} {}{id}",
                        tag.name,
                        kitter::i18n::tn(
                            "cli.assignments",
                            tag.assignments,
                            &[("count", (tag.assignments).to_string())]
                        )
                    );
                }
            }
            return Ok(());
        }
        TagAction::Create { name, parent } => {
            let parent = parent
                .map(|parent| resolve_tag(tags, &parent))
                .transpose()?;
            tags.add(&name, parent).map_err(anyhow::Error::from)?;
        }
        TagAction::Rename { tag, name } => {
            let tag = resolve_tag(tags, &tag)?;
            tags.rename(tag, &name).map_err(anyhow::Error::from)?;
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
                bail!(t!("errors.tag_reorder_invalid").to_string());
            }
        }
    }
    save_tag_states(&skill_tags, &project_tags)
}

#[derive(Serialize)]
struct ThemeListOutput {
    active: String,
    dir: PathBuf,
    themes: Vec<ThemeOutput>,
    errors: Vec<ThemeErrorOutput>,
}

#[derive(Serialize)]
struct ThemeOutput {
    id: String,
    name: String,
    builtin: bool,
    path: Option<PathBuf>,
    active: bool,
}

#[derive(Serialize)]
struct ThemeErrorOutput {
    id: String,
    path: PathBuf,
    error: String,
}

#[derive(Serialize)]
struct ThemeCheckOutput {
    id: String,
    error: Option<String>,
    problems: Vec<String>,
    /// Icons the theme replaces, by name.
    icons: BTreeMap<String, PathBuf>,
    fonts: Vec<PathBuf>,
}

#[derive(Serialize)]
struct ThemeIconOutput {
    name: &'static str,
    /// The theme's replacement, when `--theme` names one that has it.
    path: Option<PathBuf>,
    /// The replacement keeps its own colours (`icons/color/`).
    color: bool,
}

fn manage_themes(action: ThemeAction, json: bool) -> Result<()> {
    let dir = themes::themes_dir();
    let mut config = kitter::AppConfig::load()?;
    match action {
        ThemeAction::List => {
            let files = themes::list_files_in(&dir);
            let themes = themes::list_in(&dir);
            let active = themes
                .iter()
                .find(|theme| theme.id == config.theme_style)
                .map_or(themes::STANDARD, |theme| theme.id.as_str())
                .to_string();
            let output = ThemeListOutput {
                themes: themes
                    .into_iter()
                    .map(|theme| ThemeOutput {
                        active: theme.id == active,
                        builtin: theme.source == ThemeSource::Builtin,
                        path: theme.source.path().map(PathBuf::from),
                        id: theme.id,
                        name: theme.name,
                    })
                    .collect(),
                errors: files
                    .into_iter()
                    .filter_map(|file| {
                        Some(ThemeErrorOutput {
                            error: file.theme.err()?,
                            id: file.id,
                            path: file.path,
                        })
                    })
                    .collect(),
                active,
                dir,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
                return Ok(());
            }
            println!("{}", t!("cli.theme_directory", path = output.dir.display()));
            for theme in &output.themes {
                let marker = if theme.active { "*" } else { " " };
                println!("{marker} {:<24} {}", theme.id, theme.name);
            }
            for error in &output.errors {
                println!(
                    "{}",
                    t!("cli.theme_error", id = error.id, error = error.error)
                );
            }
            Ok(())
        }
        ThemeAction::Show { id } => {
            let theme = themes::find_in(&dir, &id)?;
            print!("{}", themes::to_json_text(&theme));
            Ok(())
        }
        ThemeAction::Check { theme } => {
            let path = PathBuf::from(&theme);
            let looks_like_path = theme.ends_with(".json") || path.components().count() > 1;
            let file = if looks_like_path && path.exists() {
                themes::load_path(&dir, &path)
            } else if themes::BUILTIN_THEMES.contains(&theme.as_str()) {
                println!("{}", t!("cli.theme_no_problems"));
                return Ok(());
            } else {
                match themes::load_in(&dir, &theme) {
                    Some(file) => file,
                    None => kitter::user_bail!("errors.theme_not_found", id = theme),
                }
            };
            let (icons, fonts) = file
                .theme
                .as_ref()
                .map(|theme| (theme.icons.clone(), theme.fonts.clone()))
                .unwrap_or_default();
            let error = file.theme.err();
            if json {
                let output = ThemeCheckOutput {
                    id: file.id.clone(),
                    error: error.clone(),
                    problems: file.problems.clone(),
                    icons,
                    fonts,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                for problem in &file.problems {
                    println!("{problem}");
                }
                if error.is_none() && file.problems.is_empty() {
                    println!("{}", t!("cli.theme_no_problems"));
                }
            }
            if let Some(error) = error {
                kitter::user_bail!("errors.theme_invalid", id = file.id, error = error);
            }
            Ok(())
        }
        ThemeAction::Use { id } => {
            let theme = themes::find_in(&dir, &id)?;
            config.theme_style = theme.id;
            config.save()?;
            println!("{}", t!("cli.theme_applied", name = theme.name));
            Ok(())
        }
        ThemeAction::Create { id, from, name } => {
            let from = themes::find_in(&dir, &from)?;
            let path = themes::create_in(&dir, &id, name.as_deref(), &from)?;
            println!("{}", path.display());
            Ok(())
        }
        ThemeAction::Add { path, id, force } => {
            let file = themes::add_in(&dir, &path, id.as_deref(), force)?;
            for problem in &file.problems {
                eprintln!("{problem}");
            }
            println!("{}", file.path.display());
            Ok(())
        }
        ThemeAction::Remove { id } => {
            themes::remove_in(&dir, &id)?;
            if config.theme_style == id {
                config.theme_style = themes::STANDARD.into();
                config.save()?;
            }
            println!("{}", t!("cli.theme_removed", id = id));
            Ok(())
        }
        ThemeAction::Icons { theme } => {
            let (mono, color) = match theme {
                Some(id) => {
                    let theme = themes::find_in(&dir, &id)?;
                    (theme.icons, theme.color_icons)
                }
                None => (BTreeMap::new(), BTreeMap::new()),
            };
            let icons = themes::ICON_NAMES
                .into_iter()
                .map(|name| ThemeIconOutput {
                    name,
                    path: color.get(name).or_else(|| mono.get(name)).cloned(),
                    color: color.contains_key(name),
                })
                .collect::<Vec<_>>();
            if json {
                println!("{}", serde_json::to_string_pretty(&icons)?);
                return Ok(());
            }
            for icon in icons {
                match icon.path {
                    Some(path) => println!("* {:<16} {}", icon.name, path.display()),
                    None => println!("  {}", icon.name),
                }
            }
            Ok(())
        }
    }
}

fn manage_data(set: Option<PathBuf>) -> Result<()> {
    let Some(path) = set else {
        println!("{}", kitter::data_location::current().display());
        return Ok(());
    };
    let (kind, report) = kitter::data_location::change(&path)?;
    match kind {
        kitter::data_location::DataFolder::Empty => {
            println!("{}", t!("cli.data_moved", path = path.display()))
        }
        kitter::data_location::DataFolder::Existing => {
            println!("{}", t!("cli.data_switched", path = path.display()))
        }
    }
    if !report.stale_links.is_empty() {
        eprintln!("{}", t!("cli.data_stale_links"));
        for link in report.stale_links {
            eprintln!("  {}", link.display());
        }
    }
    Ok(())
}

fn manage_presets(library: &SkillLibrary, action: PresetAction) -> Result<()> {
    let mut presets = PresetStore::open()?;
    match action {
        PresetAction::List { json } => {
            let skills = library.list()?;
            let output = presets
                .presets()
                .iter()
                .map(|preset| preset_output(&presets, preset, &skills, false))
                .collect::<Result<Vec<_>>>()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if output.is_empty() {
                println!("{}", t!("cli.no_presets"));
            } else {
                for preset in output {
                    let skills = kitter::i18n::tn(
                        "common.skill_count",
                        preset.skills.len(),
                        &[("count", preset.skills.len().to_string())],
                    );
                    let projects = kitter::i18n::tn(
                        "cli.project_count",
                        preset.projects.len(),
                        &[("count", preset.projects.len().to_string())],
                    );
                    println!(
                        "{:<28} {}",
                        preset.name,
                        t!("cli.preset_summary", skills = skills, projects = projects)
                    );
                }
            }
            Ok(())
        }
        PresetAction::Show { preset, json } => {
            let skills = library.list()?;
            let preset = presets.resolve(&preset)?.clone();
            let output = preset_output(&presets, &preset, &skills, true)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
                return Ok(());
            }
            println!("{}", output.name);
            if output.id != output.name {
                println!("{}", t!("cli.id", id = output.id));
            }
            println!("{}", t!("cli.skills_heading"));
            for skill in &output.skills {
                println!("  {skill}");
            }
            if !output.files.is_empty() {
                println!("{}", t!("cli.files_heading"));
                for path in &output.files {
                    println!("  {}", path.display());
                }
            }
            println!("{}", t!("cli.projects_heading"));
            for project in &output.projects {
                let state = if project.agents_md_modified {
                    format!("  {}", t!("cli.agents_md_edited"))
                } else if project.needs_sync {
                    format!("  {}", t!("cli.needs_sync"))
                } else {
                    String::new()
                };
                println!("  {}{state}", project.path.display());
            }
            if let Some(agents_md) = output.agents_md {
                println!("AGENTS.md:\n{agents_md}");
            }
            Ok(())
        }
        PresetAction::Create { name } => {
            let preset = presets.create(&name)?;
            println!(
                "{}",
                t!("cli.preset_created", name = preset.name, id = preset.id)
            );
            Ok(())
        }
        PresetAction::Rename { preset, name } => {
            let id = presets.resolve(&preset)?.id.clone();
            presets.rename(&id, &name)
        }
        PresetAction::Delete { preset } => {
            let id = presets.resolve(&preset)?.id.clone();
            let report = presets.delete(&id, &library.list()?)?;
            finish_batch(
                "cli.removed_installations",
                report.removed,
                failure_messages(report.failures),
            )
        }
        PresetAction::Add { preset, skills } => {
            let id = presets.resolve(&preset)?.id.clone();
            let skills = resolve_skills(library, &skills)?
                .into_iter()
                .map(|skill| skill.record.storage_name)
                .collect::<Vec<_>>();
            presets.add_skills(&id, &skills)
        }
        PresetAction::Remove { preset, skills } => {
            let id = presets.resolve(&preset)?.id.clone();
            let skills = resolve_skills(library, &skills)?
                .into_iter()
                .map(|skill| skill.record.storage_name)
                .collect::<Vec<_>>();
            presets.remove_skills(&id, &skills)
        }
        PresetAction::AgentsMd { preset, set, clear } => {
            let id = presets.resolve(&preset)?.id.clone();
            if clear {
                return presets.set_agents_md(&id, "");
            }
            let Some(path) = set else {
                print!("{}", presets.agents_md(&id)?);
                return Ok(());
            };
            let content = if path.as_os_str() == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                std::fs::read_to_string(&path)
                    .with_context(|| t!("errors.read_failed", path = path.display()).to_string())?
            };
            presets.set_agents_md(&id, &content)
        }
        PresetAction::File { action } => manage_preset_files(&mut presets, action),
        PresetAction::Install {
            preset,
            project,
            target,
            yes,
            overwrite_conflicts,
            skip_conflicts,
        } => {
            let id = presets.resolve(&preset)?.id.clone();
            let project = canonical_project(&project)?;
            let targets = target
                .iter()
                .map(|target| parse_target(target))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(anyhow::Error::msg)?;
            let skills = library.list()?;
            let mut plan = project::InstallPlan::default();
            for storage in &presets.preset(&id)?.skills {
                let skill = skills
                    .iter()
                    .find(|skill| skill.record.storage_name == *storage)
                    .with_context(|| t!("errors.skill_missing").to_string())?;
                plan.add(&project, &skill.path, &skill.record.name, &targets)?;
            }
            if !confirm_install(&plan, yes)? {
                return Ok(());
            }
            let decisions = preset_file_decisions(
                &presets,
                &id,
                &project,
                overwrite_conflicts,
                skip_conflicts,
            )?;
            let report = presets
                .install_reviewed_with_files(&id, &project, &targets, &skills, plan, &decisions)?;
            finish_batch(
                "common.installed_skills",
                report.installed,
                failure_messages(report.failures),
            )
        }
        PresetAction::Sync {
            preset,
            project,
            force,
            overwrite_conflicts,
            skip_conflicts,
        } => {
            let ids = match preset {
                Some(preset) => vec![presets.resolve(&preset)?.id.clone()],
                None => presets
                    .presets()
                    .iter()
                    .map(|preset| preset.id.clone())
                    .collect(),
            };
            let project = project.as_deref().map(canonical_project).transpose()?;
            let skills = library.list()?;
            let mut synced = 0usize;
            let mut failures = Vec::new();
            for id in ids {
                let projects = presets
                    .installations(&id)
                    .map(|installation| installation.project.clone())
                    .filter(|path| project.as_ref().is_none_or(|project| project == path))
                    .collect::<Vec<_>>();
                for path in projects {
                    let result = preset_file_decisions(
                        &presets,
                        &id,
                        &path,
                        overwrite_conflicts,
                        skip_conflicts,
                    )
                    .and_then(|decisions| {
                        presets.sync_with_files(&id, &path, &skills, force, &decisions)
                    });
                    match result {
                        Ok(report) => {
                            synced += 1;
                            failures.extend(failure_messages(report.failures));
                        }
                        Err(error) => {
                            let modified =
                                presets.status(&id, &path, &skills).is_ok_and(|status| {
                                    status.agents_md == AgentsMdState::Modified
                                        || status
                                            .files
                                            .iter()
                                            .any(|f| f.state == FileState::Modified && !f.removed)
                                });
                            let mut failure = t!(
                                "errors.item_detail",
                                item = format!("{id} → {}", path.display()),
                                error = kitter::i18n::error_diagnostic(&error)
                            )
                            .to_string();
                            if modified {
                                failure.push_str(&t!("cli.force_hint"));
                            }
                            failures.push(failure);
                        }
                    }
                }
            }
            finish_batch("cli.synced_projects", synced, failures)
        }
        PresetAction::Uninstall { preset, project } => {
            let id = presets.resolve(&preset)?.id.clone();
            let project = canonical_project(&project)?;
            let report = presets.uninstall(&id, &project, &library.list()?)?;
            finish_batch(
                "cli.removed_skills",
                report.removed,
                failure_messages(report.failures),
            )
        }
    }
}

fn manage_preset_files(presets: &mut PresetStore, action: PresetFileAction) -> Result<()> {
    match action {
        PresetFileAction::List { preset, json } => {
            let id = &presets.resolve(&preset)?.id;
            let files = presets.files(id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&files)?);
            } else {
                for path in files {
                    println!("{}", path.display());
                }
            }
            Ok(())
        }
        PresetFileAction::Read { preset, path } => {
            print!(
                "{}",
                presets.read_file(&presets.resolve(&preset)?.id, &path)?
            );
            Ok(())
        }
        PresetFileAction::Write {
            preset,
            path,
            source,
        } => {
            let id = presets.resolve(&preset)?.id.clone();
            let content = if source.as_os_str() == "-" {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes)?;
                bytes
            } else {
                std::fs::read(&source).with_context(|| {
                    t!("errors.read_failed", path = source.display()).to_string()
                })?
            };
            presets.write_file(&id, &path, &content)
        }
        PresetFileAction::Rename { preset, from, to } => {
            let id = presets.resolve(&preset)?.id.clone();
            presets.rename_file(&id, &from, &to)
        }
        PresetFileAction::Delete { preset, path } => {
            let id = presets.resolve(&preset)?.id.clone();
            presets.delete_file(&id, &path)
        }
    }
}

fn preset_file_decisions(
    presets: &PresetStore,
    id: &str,
    project: &Path,
    overwrite: bool,
    skip: bool,
) -> Result<FileDecisions> {
    let conflicts = presets.file_conflicts(id, project)?;
    if !conflicts.is_empty() && !overwrite && !skip {
        let conflict = kitter::preset::PresetFileConflicts {
            paths: conflicts.into_iter().map(|f| f.path).collect(),
        };
        let message =
            kitter::i18n::UserError::new(format!("{conflict}\n{}", t!("cli.file_conflict_hint")));
        return Err(anyhow::Error::new(conflict).context(message));
    }
    let decision = if overwrite {
        FileConflictDecision::Overwrite
    } else {
        FileConflictDecision::Skip
    };
    Ok(conflicts.into_iter().map(|f| (f.path, decision)).collect())
}

fn failure_messages(failures: Vec<SkillFailure>) -> Vec<String> {
    failures.iter().map(ToString::to_string).collect()
}

fn preset_output(
    presets: &PresetStore,
    preset: &Preset,
    skills: &[SkillSummary],
    include_agents_md: bool,
) -> Result<PresetOutput> {
    let names = preset
        .skills
        .iter()
        .filter_map(|storage_name| {
            skills
                .iter()
                .find(|skill| &skill.record.storage_name == storage_name)
                .map(|skill| skill.record.name.clone())
        })
        .collect();
    let projects = presets
        .installations(&preset.id)
        .map(|installation| {
            let status = presets.status(&preset.id, &installation.project, skills)?;
            Ok(PresetProjectOutput {
                path: installation.project.clone(),
                targets: installation.targets.clone(),
                needs_sync: status.needs_sync(),
                agents_md_modified: status.agents_md == AgentsMdState::Modified,
                files: status.files,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let agents_md = if include_agents_md {
        Some(presets.agents_md(&preset.id)?).filter(|content| !content.trim().is_empty())
    } else {
        None
    };
    Ok(PresetOutput {
        id: preset.id.clone(),
        name: preset.name.clone(),
        skills: names,
        agents_md,
        files: presets.files(&preset.id)?,
        projects,
    })
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
        .with_context(|| t!("errors.group_not_found", name = selector).to_string())
}

fn resolve_tag(tags: &TagState, selector: &str) -> Result<TagId> {
    let selector = selector.trim().trim_start_matches('#').trim();
    if let Some(id) = selector.strip_prefix("id:") {
        let id = id
            .parse::<TagId>()
            .with_context(|| t!("errors.tag_id_invalid", id = selector).to_string())?;
        return tags
            .tag(id)
            .map(|tag| tag.id)
            .with_context(|| t!("errors.tag_not_found", name = selector).to_string());
    }
    let matches = tags
        .tags()
        .iter()
        .filter(|tag| tag.name.eq_ignore_ascii_case(selector))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [tag] => Ok(tag.id),
        [] => bail!(t!("errors.tag_not_found", name = selector).to_string()),
        _ => bail!(t!("errors.tag_ambiguous", name = selector).to_string()),
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
        bail!(t!("errors.project_missing", path = path.display()).to_string());
    }
    path.canonicalize()
        .with_context(|| t!("errors.project_read_failed", path = path.display()).to_string())
}

fn same_file(left: &Path, right: &Path) -> bool {
    left.canonicalize().ok() == right.canonicalize().ok()
}

fn finish_batch(key: &str, succeeded: usize, failures: Vec<String>) -> Result<()> {
    let message = kitter::i18n::tn(key, succeeded, &[("count", succeeded.to_string())]);
    if failures.is_empty() {
        println!("{message}");
        return Ok(());
    }
    if succeeded > 0 {
        eprintln!("{message}");
    }
    bail!(kitter::i18n::tn(
        "cli.operations_failed",
        failures.len(),
        &[
            ("count", (failures.len()).to_string()),
            ("failures", (failures.join("; ")).to_string())
        ]
    ))
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
    fn parses_agents_list_and_check() {
        let cli = Cli::try_parse_from(["kitter", "agents", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Agents {
                json: true,
                action: None
            }
        ));
        for args in [
            vec![
                "kitter",
                "agents",
                "check",
                "custom.js",
                "--project",
                "/tmp/project",
                "--json",
            ],
            vec![
                "kitter",
                "agents",
                "--json",
                "check",
                "custom.js",
                "--project",
                "/tmp/project",
            ],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            let Command::Agents {
                json: true,
                action: Some(super::AgentsAction::Check { file, project }),
            } = cli.command
            else {
                panic!("expected agents check");
            };
            assert_eq!(file.unwrap(), std::path::PathBuf::from("custom.js"));
            assert_eq!(project.unwrap(), std::path::PathBuf::from("/tmp/project"));
        }
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
    fn target_parser_accepts_aliases_and_rejects_non_installable_agents() {
        for (input, expected) in [
            ("universal", "universal"),
            ("claude", "claude-code"),
            ("claude_code", "claude-code"),
            ("open_code", "opencode"),
            ("dimagent", "dimagent"),
            ("open_claw", "openclaw"),
            ("amp", "amp"),
            ("hermes", "hermes"),
        ] {
            assert_eq!(super::parse_target(input).unwrap().as_str(), expected);
        }
        for input in ["missing-plugin"] {
            let error = super::parse_target(input).unwrap_err();
            assert!(error.contains(input));
            for target in kitter::agent_plugins::AgentRegistry::global().install_targets() {
                assert!(error.contains(target.as_str()));
            }
        }
    }

    #[test]
    fn agent_parser_uses_registry_aliases() {
        for (input, expected) in [
            ("claude", "claude-code"),
            ("claude_code", "claude-code"),
            ("open_code", "opencode"),
            ("dim_agent", "dimagent"),
            ("open_claw", "openclaw"),
            ("cursor", "cursor"),
        ] {
            assert_eq!(super::parse_agent(input).unwrap().as_str(), expected);
        }
        assert!(
            super::parse_agent("missing-plugin")
                .unwrap_err()
                .contains("missing-plugin")
        );
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
            panic!("expected skill tag assignment");
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn client_settings_are_not_cli_commands() {
        assert!(Cli::try_parse_from(["kitter", "config", "set-theme", "dark"]).is_err());
        let cli = Cli::try_parse_from(["kitter", "data", "--set", "/tmp/kitter"])
            .expect("the data folder should remain configurable");
        let Command::Data { set } = cli.command else {
            panic!("expected data command");
        };
        assert_eq!(set.unwrap(), std::path::PathBuf::from("/tmp/kitter"));
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
