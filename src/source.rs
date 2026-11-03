use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::{SkillLibrary, SkillOrigin, SkillRecord};

mod git_source;
pub(crate) mod local;
pub(crate) mod remote;
mod updates;
pub use updates::{
    BatchUpdateReport, UpdateProgress, can_update, check_updates_if_due,
    update_skills_with_progress,
};

pub struct ScannedSkill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

enum ScanOrigin {
    Npx { repository: String },
    Claude { plugin: String },
    Local { root: PathBuf, label: String },
}

pub struct SkillScan {
    origin: ScanOrigin,
    skills: Vec<ScannedSkill>,
    /// Update sources of a git scan; npx scans read them from the lock file.
    git_sources: Option<git_source::Sources>,
    _temp: Option<TempDir>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped: usize,
}

impl SkillScan {
    pub fn skills(&self) -> &[ScannedSkill] {
        &self.skills
    }

    pub fn source_label(&self) -> &str {
        match &self.origin {
            ScanOrigin::Npx { repository, .. } => repository,
            ScanOrigin::Claude { plugin } => plugin,
            ScanOrigin::Local { label, .. } => label,
        }
    }

    pub fn source_key(&self) -> String {
        match &self.origin {
            ScanOrigin::Npx { repository, .. } => crate::SkillSource::Npx {
                repository: repository.clone(),
            }
            .key(),
            ScanOrigin::Claude { plugin } => crate::SkillSource::ClaudeMarketplace {
                plugin: plugin.clone(),
            }
            .key(),
            ScanOrigin::Local { root, .. } => {
                crate::SkillSource::Local { path: root.clone() }.key()
            }
        }
    }

    pub fn default_group_name(&self) -> String {
        match &self.origin {
            ScanOrigin::Npx { repository, .. } => crate::SkillSource::Npx {
                repository: repository.clone(),
            }
            .label(),
            ScanOrigin::Claude { plugin } => crate::SkillSource::ClaudeMarketplace {
                plugin: plugin.clone(),
            }
            .label(),
            ScanOrigin::Local { root, .. } => {
                crate::SkillSource::Local { path: root.clone() }.label()
            }
        }
    }

    pub fn import_selected(
        self,
        library: &mut SkillLibrary,
        selected: &HashSet<String>,
        group_name: Option<&str>,
    ) -> Result<ImportSummary> {
        self.import_selected_with_mode(library, selected, group_name, crate::LocalInstallMode::Link)
    }

    /// The mode applies only to local folders. Existing identities are skipped.
    pub fn import_selected_with_mode(
        self,
        library: &mut SkillLibrary,
        selected: &HashSet<String>,
        group_name: Option<&str>,
        install_mode: crate::LocalInstallMode,
    ) -> Result<ImportSummary> {
        if selected.is_empty() {
            crate::user_bail!("errors.skill_selection_required");
        }
        let SkillScan {
            origin,
            skills,
            mut git_sources,
            _temp: scan_temp,
        } = self;
        let from_lock = matches!(origin, ScanOrigin::Npx { .. }) && git_sources.is_none();
        let discovered_skills = skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>();
        // The scan already downloaded a complete snapshot. Import from that
        // snapshot only; update workspaces are prepared lazily by update flows.
        let mut scan_hashes = if from_lock {
            npx_lock_hashes(
                scan_temp
                    .as_ref()
                    .context(crate::user_error!("errors.scan_required"))?
                    .path(),
            )?
        } else {
            HashMap::new()
        };
        let mut scan_sources = if from_lock {
            npx_lock_entries(
                scan_temp
                    .as_ref()
                    .context(crate::user_error!("errors.scan_required"))?
                    .path(),
            )?
        } else {
            HashMap::new()
        };
        let mut added_skills = Vec::new();
        let mut skipped = 0;
        let group_name = group_name
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string);
        let mut group_id = None;
        for skill in skills
            .into_iter()
            .filter(|skill| selected.contains(&skill.name))
        {
            let identity = match &origin {
                ScanOrigin::Npx { repository, .. } => SkillOrigin::Npx {
                    repository: repository.clone(),
                    skill: skill.name.clone(),
                    source_hash: None,
                }
                .identity_key(&skill.name),
                ScanOrigin::Claude { plugin } => SkillOrigin::ClaudeMarketplace {
                    plugin: plugin.clone(),
                    skill: skill.name.clone(),
                }
                .identity_key(&skill.name),
                ScanOrigin::Local { root, .. } => SkillOrigin::Local {
                    path: skill.path.clone(),
                    source_root: Some(root.clone()),
                    install_mode,
                    source_hash: None,
                }
                .identity_key(&skill.name),
            };
            if library.contains_identity(&identity) {
                skipped += 1;
                continue;
            }
            let (source_path, skill_origin) = match &origin {
                ScanOrigin::Npx { repository } => (
                    skill.path.clone(),
                    SkillOrigin::Npx {
                        repository: repository.clone(),
                        skill: skill.name.clone(),
                        source_hash: scan_hashes.remove(&skill.name),
                    },
                ),
                ScanOrigin::Claude { plugin } => (
                    skill.path.clone(),
                    SkillOrigin::ClaudeMarketplace {
                        plugin: plugin.clone(),
                        skill: skill.name.clone(),
                    },
                ),
                ScanOrigin::Local { root, .. } => (
                    skill.path.clone(),
                    SkillOrigin::Local {
                        path: skill.path.clone(),
                        source_root: Some(root.clone()),
                        install_mode,
                        source_hash: None,
                    },
                ),
            };
            if group_id.is_none()
                && let Some(group_name) = group_name.as_deref()
            {
                group_id = Some(library.ensure_group(group_name)?);
            }
            let name = skill.name.clone();
            library.import(
                &source_path,
                SkillRecord {
                    name: skill.name,
                    storage_name: String::new(),
                    description: skill.description,
                    custom_description: None,
                    origin: skill_origin,
                    update_available: false,
                    group_id: group_id.clone(),
                    last_operated_at: 0,
                },
            )?;
            if let ScanOrigin::Npx { repository } = &origin
                && let Some(entry) = scan_sources.remove(&name)
                && let Some(source) = entry.remote_source(repository)
            {
                library.set_remote_skill_source(identity, source)?;
            } else if let Some(source) = git_sources
                .as_mut()
                .and_then(|sources| sources.remove(&name))
            {
                library.set_remote_skill_source(identity, source)?;
            }
            added_skills.push(name);
        }
        let source = match origin {
            ScanOrigin::Npx { repository, .. } => crate::SkillSource::Npx { repository },
            ScanOrigin::Claude { plugin } => crate::SkillSource::ClaudeMarketplace { plugin },
            ScanOrigin::Local { root, .. } => crate::SkillSource::Local { path: root },
        };
        let added = added_skills.len();
        library.record_source(source, discovered_skills, added_skills)?;
        Ok(ImportSummary { added, skipped })
    }
}

/// Find every Skill below a user-selected folder. Hidden directories are skipped at
/// every depth so repository metadata and tool caches never appear as candidates.
pub fn scan_local(root: &Path) -> Result<SkillScan> {
    if !root.is_dir() {
        crate::user_bail!("errors.folder_required");
    }
    let root = root.canonicalize()?;

    let mut skill_dirs = BTreeSet::new();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        })
    {
        let entry = entry
            .with_context(|| t!("errors.folder_read_failed", path = root.display()).to_string())?;
        if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
            if let Some(parent) = entry.path().parent() {
                skill_dirs.insert(parent.to_path_buf());
            }
        }
    }

    let mut skills = Vec::new();
    let mut names = HashSet::new();
    for path in skill_dirs {
        let (front_name, description) = crate::library::read_frontmatter(&path.join("SKILL.md"))?;
        let name = front_name
            .or_else(|| {
                path.file_name()
                    .map(|value| value.to_string_lossy().into_owned())
            })
            .context(crate::user_error!("errors.skill_name_unreadable"))?;
        if !names.insert(name.clone()) {
            bail!(
                crate::user_error!("errors.skill_name_conflict_notice")
                    .detail(t!("errors.skill_name_conflict", name = name).to_string())
            );
        }
        skills.push(ScannedSkill {
            name,
            description: description.unwrap_or_default(),
            path,
        });
    }

    if skills.is_empty() {
        crate::user_bail!("errors.folder_empty");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Local {
            root: root.to_path_buf(),
            label: root.display().to_string(),
        },
        skills,
        git_sources: None,
        _temp: None,
    })
}

pub fn scan_npx(input: &str) -> Result<SkillScan> {
    let repository = normalize_npx_source(input)?;
    match git_source::Fetcher::detect()? {
        git_source::Fetcher::Npx => scan_with_npx(repository),
        git_source::Fetcher::Git => scan_with_git(repository),
    }
}

fn scan_with_git(repository: String) -> Result<SkillScan> {
    let target = git_source::parse_target(&repository)?;
    let temp = TempDir::new()?;
    let checkout = temp.path().join("repo");
    git_source::checkout(&target.source, &checkout)?;
    let mut skills = Vec::new();
    let mut sources = git_source::Sources::new();
    for (skill, source) in git_source::discover(&checkout, &target.source, target.skill.as_deref())?
    {
        sources.insert(skill.name.clone(), source);
        skills.push(skill);
    }
    if skills.is_empty() {
        crate::user_bail!("errors.source_address_empty");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Npx { repository },
        skills,
        git_sources: Some(sources),
        _temp: Some(temp),
    })
}

fn scan_with_npx(repository: String) -> Result<SkillScan> {
    let temp = TempDir::new()?;
    npx_add(temp.path(), &repository, "*")?;
    let root = temp.path().join(".agents/skills");
    let mut skills = Vec::new();
    for entry in fs::read_dir(&root).with_context(|| {
        crate::user_error!("errors.source_empty")
            .detail(t!("errors.source_empty_detail", source = repository).to_string())
    })? {
        let path = entry?.path();
        if !path.is_dir() || !path.join("SKILL.md").is_file() {
            continue;
        }
        let (front_name, description) = crate::library::read_frontmatter(&path.join("SKILL.md"))?;
        let skill = front_name
            .or_else(|| {
                path.file_name()
                    .map(|value| value.to_string_lossy().into_owned())
            })
            .context(crate::user_error!("errors.skill_name_unreadable"))?;
        skills.push(ScannedSkill {
            name: skill,
            description: description.unwrap_or_default(),
            path,
        });
    }
    if skills.is_empty() {
        crate::user_bail!("errors.source_address_empty");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Npx { repository },
        skills,
        git_sources: None,
        _temp: Some(temp),
    })
}

pub fn scan_claude(input: &str) -> Result<SkillScan> {
    let plugin = normalize_claude_plugin(input)?;
    run(tool_command("claude").args(["plugin", "install", &plugin]))?;
    let mut skills = Vec::new();
    for path in find_claude_skills(&plugin)? {
        let (front_name, description) = crate::library::read_frontmatter(&path.join("SKILL.md"))?;
        let skill = front_name
            .or_else(|| {
                path.file_name()
                    .map(|value| value.to_string_lossy().into_owned())
            })
            .context(crate::user_error!("errors.skill_name_unreadable"))?;
        skills.push(ScannedSkill {
            name: skill,
            description: description.unwrap_or_default(),
            path,
        });
    }
    if skills.is_empty() {
        crate::user_bail!("errors.plugin_empty");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Claude { plugin },
        skills,
        git_sources: None,
        _temp: None,
    })
}

fn normalize_npx_source(input: &str) -> Result<String> {
    let value = input.trim();
    if value.is_empty() {
        crate::user_bail!("errors.source_address_required");
    }
    if value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("git@")
        || value.starts_with("ssh://")
    {
        return Ok(value.to_string());
    }
    let parts = value.split_whitespace().collect::<Vec<_>>();
    parts
        .windows(2)
        .find_map(|pair| (pair[0] == "add").then(|| pair[1].to_string()))
        .or_else(|| {
            parts
                .iter()
                .find(|part| part.starts_with("http"))
                .map(|part| (*part).to_string())
        })
        .context(crate::user_error!("errors.source_address_invalid"))
}

fn normalize_claude_plugin(input: &str) -> Result<String> {
    let value = input.trim();
    if value.is_empty() {
        crate::user_bail!("errors.plugin_name_required");
    }
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 1 {
        return Ok(parts[0].to_string());
    }
    parts
        .windows(2)
        .find_map(|pair| (pair[0] == "install").then(|| pair[1].to_string()))
        .context(crate::user_error!("errors.plugin_name_invalid"))
}

pub fn update(library: &mut SkillLibrary, name: &str) -> Result<()> {
    let record = library.record(name)?;
    update_record(library, record)
}

pub fn update_by_storage(library: &mut SkillLibrary, storage_name: &str) -> Result<()> {
    let record = library.record_by_storage(storage_name)?;
    update_record(library, record)
}

fn update_record(library: &mut SkillLibrary, mut record: SkillRecord) -> Result<()> {
    let storage_name = record.storage_name.clone();
    if library.is_linked_source(&storage_name) {
        crate::user_bail!("errors.linked_update_required");
    }
    if matches!(
        record.origin,
        SkillOrigin::Local {
            install_mode: crate::LocalInstallMode::Link,
            ..
        }
    ) {
        let candidate = crate::adoption::AdoptionCandidate::from_local_record(&record)?;
        library.adopt(&candidate, &[])?;
        return Ok(());
    }
    let temp = TempDir::new()?;
    let mut updated_origin = None;
    let source = match record.origin.clone() {
        SkillOrigin::Builtin => crate::user_bail!("errors.builtin_update_managed"),
        SkillOrigin::Npx {
            repository, skill, ..
        } if git_source::Fetcher::detect()? == git_source::Fetcher::Git => {
            let path = update_with_git(library, &record, &repository, &skill, temp.path())?;
            updated_origin = Some(SkillOrigin::Npx {
                repository,
                skill,
                source_hash: None,
            });
            path
        }
        SkillOrigin::Npx {
            repository, skill, ..
        } => {
            let workspace = npx_workspace_in(library.data_dir(), &repository);
            // Re-add exactly this skill. `skills update` can update the whole
            // workspace and has reported success even when reinstalls failed.
            npx_add(&workspace, &repository, &skill)?;
            let source_hash = npx_lock_hash(&workspace, &skill)?;
            updated_origin = Some(SkillOrigin::Npx {
                repository,
                skill: skill.clone(),
                source_hash,
            });
            npx_skill_path(&workspace, &skill)
        }
        SkillOrigin::ClaudeMarketplace { plugin, skill } => {
            run(tool_command("claude").args(["plugin", "update", &plugin]))?;
            find_claude_skill(&plugin, &skill)?
        }
        SkillOrigin::Git { repository, subdir } => {
            run(tool_command("git")
                .args(["clone", "--depth", "1", &repository])
                .arg(temp.path().join("repo")))?;
            find_skill_dir(
                &subdir
                    .as_ref()
                    .map(|p| temp.path().join("repo").join(p))
                    .unwrap_or_else(|| temp.path().join("repo")),
            )?
        }
        SkillOrigin::Local {
            path,
            source_root,
            install_mode,
            ..
        } => {
            if !path.try_exists()? {
                return Ok(());
            }
            if !path.join("SKILL.md").is_file() {
                crate::user_bail!("errors.skill_marker_missing");
            }
            let snapshot = temp.path().join("local");
            local::copy_tree(&path, &snapshot)?;
            updated_origin = Some(SkillOrigin::Local {
                path,
                source_root,
                install_mode,
                source_hash: Some(local::tree_hash(&snapshot)?),
            });
            snapshot
        }
        SkillOrigin::Unknown => crate::user_bail!("errors.update_source_missing"),
    };
    // Downloading may take seconds. Keep changes made to the library meanwhile,
    // and never resurrect a skill removed or changed during the download.
    *library = SkillLibrary::open_in(library.data_dir())?;
    let latest = library.record_by_storage(&storage_name)?;
    if latest.identity_key() != record.identity_key()
        || latest.last_operated_at != record.last_operated_at
    {
        crate::user_bail!("errors.skill_changed_notice");
    }
    record = latest;
    if let Some(origin) = updated_origin {
        record.origin = origin;
    }
    record.update_available = false;
    library.replace_by_storage(&source, storage_name, record)
}

/// Check out the skill's recorded source and return its folder.
fn update_with_git(
    library: &SkillLibrary,
    record: &SkillRecord,
    repository: &str,
    skill: &str,
    temp: &Path,
) -> Result<PathBuf> {
    let source = match library.remote_skill_source(record) {
        Some(source) => source.clone(),
        None => match legacy_npx_source(library, repository, skill) {
            Some(source) => source,
            None => git_source::parse_target(repository)?.source,
        },
    };
    let checkout = temp.join("repo");
    git_source::checkout(&source, &checkout)?;
    git_source::locate(&checkout, &source, skill)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateCheckProgress {
    pub scanned: usize,
    pub total: usize,
    pub current: Option<String>,
}

pub fn check_updates(library: &mut SkillLibrary) -> Result<usize> {
    check_updates_with_progress(library, |_| {})
}

pub fn check_updates_with_progress(
    library: &mut SkillLibrary,
    mut on_progress: impl FnMut(UpdateCheckProgress),
) -> Result<usize> {
    let records = library
        .list()?
        .into_iter()
        .filter(|skill| {
            !skill.record.origin.is_builtin()
                && !library.is_linked_source(&skill.record.storage_name)
        })
        .map(|skill| skill.record)
        .collect::<Vec<_>>();
    let total = records.len();
    let mut scanned = 0usize;
    on_progress(UpdateCheckProgress {
        scanned,
        total,
        current: None,
    });

    let mut groups = BTreeMap::<remote::RemoteKey, Vec<remote::CheckSkill>>::new();
    let mut other_records = Vec::new();
    let mut failures = Vec::new();
    let mut failed_names = Vec::new();
    for record in records {
        if matches!(
            record.origin,
            SkillOrigin::Git { .. } | SkillOrigin::Npx { .. }
        ) {
            let name = record.name.clone();
            match remote::prepare(library, record) {
                Ok((key, skill)) => groups.entry(key).or_default().push(skill),
                Err(error) => {
                    failed_names.push(name.clone());
                    failures.push(format!("{name}: {}", crate::i18n::error_diagnostic(&error)));
                    scanned += 1;
                    on_progress(UpdateCheckProgress {
                        scanned,
                        total,
                        current: Some(name),
                    });
                }
            }
        } else {
            other_records.push(record);
        }
    }
    let groups = groups.into_iter().collect::<Vec<_>>();
    let data_dir = library.data_dir().to_path_buf();
    let mut results = Vec::new();
    // One job per repository/ref, at most eight concurrent network operations.
    // Workers never touch the library; the caller publishes results/progress.
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let (sender, receiver) = std::sync::mpsc::channel();
        for _ in 0..groups.len().min(8) {
            let sender = sender.clone();
            let groups = &groups;
            let next = &next;
            let data_dir = &data_dir;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((key, skills)) = groups.get(index) else {
                        break;
                    };
                    let result = remote::check_group(data_dir, key, skills);
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        for (index, result) in receiver {
            let (_, skills) = &groups[index];
            match result {
                Ok(available) => {
                    for (skill, available) in skills.iter().zip(available) {
                        match available {
                            Ok(available) => results.push((skill.record.clone(), available)),
                            Err(error) => {
                                failed_names.push(skill.record.name.clone());
                                failures.push(format!(
                                    "{}: {}",
                                    skill.record.name,
                                    crate::i18n::error_diagnostic(&error)
                                ));
                            }
                        }
                        scanned += 1;
                        on_progress(UpdateCheckProgress {
                            scanned,
                            total,
                            current: Some(skill.record.name.clone()),
                        });
                    }
                }
                Err(error) => {
                    for skill in skills {
                        failed_names.push(skill.record.name.clone());
                        failures.push(format!(
                            "{}: {}",
                            skill.record.name,
                            crate::i18n::error_diagnostic(&error)
                        ));
                        scanned += 1;
                        on_progress(UpdateCheckProgress {
                            scanned,
                            total,
                            current: Some(skill.record.name.clone()),
                        });
                    }
                }
            }
        }
    });
    for record in other_records {
        match check_one(library, &record) {
            Ok(available) => results.push((record.clone(), available)),
            Err(error) => {
                failed_names.push(record.name.clone());
                failures.push(format!(
                    "{}: {}",
                    record.name,
                    crate::i18n::error_diagnostic(&error)
                ));
            }
        }
        scanned += 1;
        on_progress(UpdateCheckProgress {
            scanned,
            total,
            current: Some(record.name),
        });
    }
    let count = library.apply_update_results(&results)?;
    on_progress(UpdateCheckProgress {
        scanned,
        total,
        current: None,
    });
    if !failures.is_empty() {
        failed_names.sort();
        let names = failed_names
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(t!("errors.separator").as_ref());
        let names = if failed_names.len() > 2 {
            t!(
                "updates.more_skills",
                skills = names,
                count = failed_names.len() - 2
            )
            .to_string()
        } else {
            names
        };
        bail!(
            crate::user_error!("errors.update_check_failed", skills = names).detail(
                t!(
                    "errors.update_check_detail",
                    failures = failures.join(t!("errors.separator").as_ref())
                )
                .to_string()
            )
        );
    }
    Ok(count)
}

fn check_one(library: &SkillLibrary, record: &SkillRecord) -> Result<bool> {
    let temp = TempDir::new()?;
    let source = match &record.origin {
        SkillOrigin::Builtin => return Ok(false),
        SkillOrigin::Git { .. } => bail!(t!("errors.update_source_missing").to_string()),
        SkillOrigin::ClaudeMarketplace { plugin, skill } => find_claude_skill(plugin, skill)?,
        SkillOrigin::Local {
            path,
            install_mode: crate::LocalInstallMode::Copy,
            source_hash,
            ..
        } => {
            if !path.try_exists()? {
                return Ok(false);
            }
            let current = local::tree_hash(path)?;
            return Ok(source_hash.as_ref().is_some_and(|hash| hash != &current));
        }
        SkillOrigin::Local { path, .. } => path.clone(),
        SkillOrigin::Unknown => return Ok(false),
        SkillOrigin::Npx { .. } => return Ok(false),
    };
    let library_path = library.skill_path_by_storage(&record.storage_name)?;
    if let Some(original) = library.trigger_source_by_storage(&record.storage_name) {
        let comparison = temp.path().join("library-source");
        crate::library::copy_tree(&library_path, &comparison)?;
        original.write(&comparison)?;
        Ok(!same_tree(&source, &comparison)?)
    } else {
        Ok(!same_tree(&source, &library_path)?)
    }
}

#[derive(Debug, Deserialize)]
struct NpxLockFile {
    #[serde(default)]
    skills: HashMap<String, NpxLockEntry>,
}

#[derive(Debug, Deserialize)]
struct NpxLockEntry {
    #[serde(rename = "computedHash", default)]
    computed_hash: Option<String>,
    #[serde(rename = "skillFolderHash", default)]
    skill_folder_hash: Option<String>,
    #[serde(rename = "sourceUrl", default)]
    source_url: Option<String>,
    #[serde(rename = "skillPath", default)]
    skill_path: Option<String>,
    #[serde(rename = "ref", default)]
    reference: Option<String>,
}

impl NpxLockEntry {
    fn remote_source(&self, fallback: &str) -> Option<remote::RemoteSkill> {
        // Installer lock files can rewrite SSH URLs to HTTPS. Keep the user's
        // repository transport, while retaining the resolved ref and skill path.
        let mut source = remote::RemoteSkill::parse(fallback)
            .ok()
            .or_else(|| remote::RemoteSkill::parse(self.source_url.as_deref()?).ok())?;
        if self.reference.is_some() {
            source.reference = self.reference.clone();
        }
        if let Some(path) = &self.skill_path {
            source.skill_path = Some(
                path.strip_suffix("SKILL.md")
                    .unwrap_or(path)
                    .trim_end_matches('/')
                    .to_owned(),
            );
        }
        Some(source)
    }

    fn hash(self) -> Option<String> {
        self.skill_folder_hash.or(self.computed_hash)
    }
}

fn npx_workspace_in(data_dir: &Path, repository: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    repository.hash(&mut hasher);
    data_dir
        .join("npx-sources")
        .join(format!("{:016x}", hasher.finish()))
}

fn npx_skill_path(workspace: &Path, skill: &str) -> PathBuf {
    workspace.join(".agents").join("skills").join(skill)
}

fn npx_add(workspace: &Path, repository: &str, skill: &str) -> Result<()> {
    fs::create_dir_all(workspace)?;
    run(tool_command("npx").current_dir(workspace).args([
        "-y",
        "skills",
        "add",
        repository,
        "--skill",
        skill,
        "--agent",
        "universal",
        "--yes",
        "--copy",
    ]))
}

fn npx_lock_hash(workspace: &Path, skill: &str) -> Result<Option<String>> {
    Ok(npx_lock_hashes(workspace)?.remove(skill))
}

fn npx_lock_hashes(workspace: &Path) -> Result<HashMap<String, String>> {
    Ok(npx_lock_entries(workspace)?
        .into_iter()
        .filter_map(|(name, entry)| entry.hash().map(|hash| (name, hash)))
        .collect())
}

fn npx_lock_entries(workspace: &Path) -> Result<HashMap<String, NpxLockEntry>> {
    let paths = [
        workspace.join(".agents/.skill-lock.json"),
        workspace.join("skills-lock.json"),
    ];
    let mut entries = HashMap::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).with_context(|| {
            t!("errors.npx_lock_read_failed", path = path.display()).to_string()
        })?;
        let lock: NpxLockFile = serde_json::from_slice(&bytes)
            .with_context(|| t!("errors.npx_lock_invalid", path = path.display()).to_string())?;
        entries.extend(lock.skills);
    }
    Ok(entries)
}

fn legacy_npx_source(
    library: &SkillLibrary,
    repository: &str,
    skill: &str,
) -> Option<remote::RemoteSkill> {
    npx_lock_entries(&npx_workspace_in(library.data_dir(), repository))
        .ok()?
        .get(skill)?
        .remote_source(repository)
}

fn same_tree(left: &Path, right: &Path) -> Result<bool> {
    let collect = |root: &Path| -> Result<Vec<(PathBuf, Vec<u8>)>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(root).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry.path().strip_prefix(root)?.to_path_buf();
            if relative
                .components()
                .next()
                .is_some_and(|part| part.as_os_str() == ".git")
            {
                continue;
            }
            files.push((relative, fs::read(entry.path())?));
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(files)
    };
    Ok(collect(left)? == collect(right)?)
}

pub(crate) fn run(command: &mut Command) -> Result<()> {
    run_output(command).map(|_| ())
}

pub(crate) fn run_output(command: &mut Command) -> Result<std::process::Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().with_context(|| {
        crate::user_error!("errors.tool_unavailable")
            .detail(t!("errors.tool_start_failed", program = program).to_string())
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!(
            crate::user_error!("errors.fetch_failed")
                .detail(t!("errors.command_failed", detail = detail).to_string())
        );
    }
    Ok(output)
}

pub(crate) fn tool_command(name: &str) -> Command {
    let executable = find_tool(name).unwrap_or_else(|| PathBuf::from(name));
    let mut command = Command::new(&executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    if let Some(bin_dir) = executable.parent() {
        let mut paths = vec![bin_dir.to_path_buf()];
        if let Some(current) = env::var_os("PATH") {
            paths.extend(env::split_paths(&current));
        }
        if let Ok(path) = env::join_paths(paths) {
            command.env("PATH", path);
        }
    }
    command
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let names = tool_names(name);
    let from_path = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|candidate| candidate.is_file())
    });
    if from_path.is_some() {
        return from_path;
    }

    let mut candidates = Vec::new();
    #[cfg(windows)]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = env::var_os(variable).map(PathBuf::from) {
                candidates.extend(names.iter().map(|name| base.join("nodejs").join(name)));
                candidates.extend(
                    names
                        .iter()
                        .map(|name| base.join("Git").join("cmd").join(name)),
                );
            }
        }
        if let Some(app_data) = env::var_os("APPDATA").map(PathBuf::from) {
            candidates.extend(names.iter().map(|name| app_data.join("npm").join(name)));
        }
    }
    #[cfg(not(windows))]
    {
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ]);
        candidates = candidates
            .into_iter()
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .collect();
    }
    if let Some(home) = dirs::home_dir() {
        candidates.extend(names.iter().map(|name| home.join(".local/bin").join(name)));
        let nvm = home.join(".nvm/versions/node");
        if let Ok(versions) = fs::read_dir(nvm) {
            candidates.extend(versions.flatten().flat_map(|entry| {
                names
                    .iter()
                    .map(move |name| entry.path().join("bin").join(name))
            }));
        }
    }
    candidates
        .into_iter()
        .filter(|candidate| candidate.is_file())
        .max_by_key(|candidate| {
            candidate
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
}

fn tool_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        return match name {
            "npx" => ["npx.cmd", "npx.exe", "npx"].map(str::to_string).to_vec(),
            "npm" => ["npm.cmd", "npm.exe", "npm"].map(str::to_string).to_vec(),
            "git" => ["git.exe", "git.cmd", "git"].map(str::to_string).to_vec(),
            "claude" => ["claude.cmd", "claude.exe", "claude"]
                .map(str::to_string)
                .to_vec(),
            _ => vec![name.to_string()],
        };
    }
    #[cfg(not(windows))]
    vec![name.to_string()]
}

fn find_skill_dir(root: &Path) -> Result<PathBuf> {
    if root.join("SKILL.md").is_file() {
        return Ok(root.to_path_buf());
    }
    WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .find(|e| e.file_name() == "SKILL.md")
        .and_then(|e| e.path().parent().map(Path::to_path_buf))
        .context(crate::user_error!("errors.repository_skill_missing"))
}

fn find_claude_skill(plugin: &str, skill: &str) -> Result<PathBuf> {
    let mut candidates = find_claude_skills(plugin)?
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == skill))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| fs::metadata(path).and_then(|m| m.modified()).ok());
    candidates
        .pop()
        .context(crate::user_error!("errors.plugin_skill_missing"))
}

fn find_claude_skills(plugin: &str) -> Result<Vec<PathBuf>> {
    let root = dirs::home_dir()
        .context(crate::user_error!("errors.home_unavailable"))?
        .join(".claude/plugins/cache");
    let mut candidates = WalkDir::new(root)
        .max_depth(7)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            let belongs_to_plugin = plugin.is_empty()
                || plugin
                    .split('@')
                    .filter(|part| !part.is_empty())
                    .all(|part| {
                        entry
                            .path()
                            .components()
                            .any(|component| component.as_os_str() == part)
                    });
            entry.file_type().is_dir()
                && entry.path().join("SKILL.md").is_file()
                && belongs_to_plugin
        })
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| fs::metadata(path).and_then(|m| m.modified()).ok());
    if candidates.is_empty() {
        crate::user_bail!("errors.claude_plugin_empty");
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(path: &Path, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: fixture\n---\n"),
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn npx_scan_is_the_only_download_even_after_partial_import_and_deletion() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let snapshot = temp.path().join("snapshot");
        for index in 1..=33 {
            let name = format!("skill-{index:02}");
            write_skill(&snapshot.join(".agents/skills").join(&name), &name);
        }
        let entries = (1..=33)
            .map(|index| {
                (
                    format!("skill-{index:02}"),
                    serde_json::json!({"computedHash": format!("hash-{index}")}),
                )
            })
            .collect::<BTreeMap<_, _>>();
        fs::write(
            snapshot.join("skills-lock.json"),
            serde_json::to_vec(&serde_json::json!({"skills": entries})).unwrap(),
        )
        .unwrap();
        let stub = bin.join("npx");
        fs::write(
            &stub,
            r#"#!/bin/sh
printf 'call\n' >> "$KITTER_IMPORT_FIXTURE/calls"
[ "$4" = 'git@github.com:fixture/import-test.git' ] || exit 90
[ "$6" = '*' ] || exit 91
/bin/cp -R "$KITTER_IMPORT_FIXTURE/snapshot/." .
"#,
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // Run in a child so PATH and the application data directory cannot
        // interfere with parallel tests or the user's installation.
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "source::tests::npx_import_fixture_child",
                "--nocapture",
            ])
            .env("KITTER_IMPORT_FIXTURE", temp.path())
            .env("KITTER_HOME", temp.path().join("data"))
            .env("PATH", &bin)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("calls"))
                .unwrap()
                .lines()
                .count(),
            2,
            "exactly one download per scan, none during import"
        );
    }

    #[cfg(unix)]
    #[test]
    fn npx_import_fixture_child() {
        let Some(root) = env::var_os("KITTER_IMPORT_FIXTURE").map(PathBuf::from) else {
            return;
        };
        let repository = "git@github.com:fixture/import-test.git";
        let mut scan = scan_npx(repository).unwrap();
        let selected = scan
            .skills()
            .iter()
            .map(|s| s.name.clone())
            .collect::<HashSet<_>>();
        // A failed eighteenth item leaves the same 17 persisted skills as an
        // interrupted import, without killing a worker or touching real data.
        fs::remove_dir_all(&scan.skills[17].path).unwrap();
        let mut library = SkillLibrary::open_in(root.join("data")).unwrap();
        assert!(
            scan.import_selected(&mut library, &selected, Some("fixture"))
                .is_err()
        );
        assert_eq!(library.list().unwrap().len(), 18); // 17 imported + built-in.
        let group = library
            .groups()
            .into_iter()
            .find(|g| g.name == "fixture")
            .unwrap();
        assert_eq!(library.delete_group(&group.id, true).unwrap().len(), 17);
        assert_eq!(library.list().unwrap().len(), 1); // Built-in skill only.
        scan = scan_npx(repository).unwrap();
        let started = std::time::Instant::now();
        let summary = scan
            .import_selected(&mut library, &selected, Some("fixture"))
            .unwrap();
        eprintln!("33-skill local import: {:?}", started.elapsed());
        assert_eq!(
            summary,
            ImportSummary {
                added: 33,
                skipped: 0
            }
        );
        assert!(
            matches!(library.record("skill-18").unwrap().origin, SkillOrigin::Npx { source_hash: Some(hash), .. } if hash == "hash-18")
        );
        assert!(!root.join("data/npx-sources").exists());
    }

    #[cfg(windows)]
    #[test]
    fn background_batch_commands_have_no_console_and_capture_output() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("console-check.cmd");
        let probe = temp.path().join("console.ps1");
        fs::write(&probe, r#"Add-Type -Name ConsoleProbe -Namespace Test -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();'
[Test.ConsoleProbe]::GetConsoleWindow().ToInt64()
"#).unwrap();
        fs::write(
            &script,
            format!(
                "@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File \"{}\"\r\n",
                probe.display()
            ),
        )
        .unwrap();
        let output = tool_command(script.to_str().unwrap()).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }

    #[test]
    fn missing_library_link_can_be_readded_and_stale_records_can_be_removed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        write_skill(&root.join("demo"), "demo");
        let data = temp.path().join("data");
        let mut library = SkillLibrary::open_in(data.clone()).unwrap();
        let selected = HashSet::from(["demo".to_string()]);
        scan_local(&root)
            .unwrap()
            .import_selected(&mut library, &selected, Some("Keep group"))
            .unwrap();
        let before = library.record("demo").unwrap();
        let destination = library.config.library_dir.join(&before.storage_name);
        crate::directory_link::remove(
            &destination,
            crate::directory_link::inspect(&destination)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let summary = scan_local(&root)
            .unwrap()
            .import_selected(&mut library, &selected, None)
            .unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 1,
                skipped: 0
            }
        );
        let after = library.record("demo").unwrap();
        assert_eq!(after.storage_name, before.storage_name);
        assert_eq!(after.group_id, before.group_id);
        assert_eq!(
            destination.canonicalize().unwrap(),
            root.join("demo").canonicalize().unwrap()
        );
        crate::directory_link::remove(
            &destination,
            crate::directory_link::inspect(&destination)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let mut library = SkillLibrary::open_in(data.clone()).unwrap();
        library.remove("demo").unwrap();
        let library = SkillLibrary::open_in(data).unwrap();
        assert!(library.record_by_storage(&before.storage_name).is_err());
        assert!(root.join("demo/SKILL.md").is_file());
        assert!(
            library
                .source_records()
                .iter()
                .all(|source| !source.added_skills.contains(&"demo".to_string()))
        );
    }

    #[test]
    fn readd_does_not_overwrite_unknown_files_at_a_stale_destination() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        write_skill(&root.join("demo"), "demo");
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        let selected = HashSet::from(["demo".to_string()]);
        scan_local(&root)
            .unwrap()
            .import_selected(&mut library, &selected, None)
            .unwrap();
        let destination = library
            .config
            .library_dir
            .join(library.record("demo").unwrap().storage_name);
        crate::directory_link::remove(
            &destination,
            crate::directory_link::inspect(&destination)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "user content").unwrap();
        assert!(
            scan_local(&root)
                .unwrap()
                .import_selected(&mut library, &selected, None)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "user content"
        );
    }

    #[test]
    fn local_batch_skips_existing_identity_and_imports_the_rest() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("source");
        write_skill(&source_root.join("alpha"), "alpha");
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();

        let first = scan_local(&source_root).unwrap();
        let first_selected = HashSet::from(["alpha".to_string()]);
        assert_eq!(
            first
                .import_selected(&mut library, &first_selected, None)
                .unwrap(),
            ImportSummary {
                added: 1,
                skipped: 0,
            }
        );

        write_skill(&source_root.join("beta"), "beta");
        let second = scan_local(&source_root).unwrap();
        let selected = HashSet::from(["alpha".to_string(), "beta".to_string()]);
        assert_eq!(
            second
                .import_selected(&mut library, &selected, None)
                .unwrap(),
            ImportSummary {
                added: 1,
                skipped: 1,
            }
        );

        let names = library
            .list()
            .unwrap()
            .into_iter()
            .map(|skill| skill.record.name)
            .collect::<HashSet<_>>();
        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
    }

    #[test]
    fn skipped_batch_does_not_create_an_empty_group() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("source");
        write_skill(&source_root.join("alpha"), "alpha");
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        let selected = HashSet::from(["alpha".to_string()]);

        scan_local(&source_root)
            .unwrap()
            .import_selected(&mut library, &selected, None)
            .unwrap();
        let summary = scan_local(&source_root)
            .unwrap()
            .import_selected(&mut library, &selected, Some("owner/repository"))
            .unwrap();

        assert_eq!(
            summary,
            ImportSummary {
                added: 0,
                skipped: 1,
            }
        );
        assert!(library.groups().is_empty());
    }

    #[test]
    fn check_updates_reports_per_skill_progress() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("source");
        write_skill(&source_root.join("alpha"), "alpha");
        write_skill(&source_root.join("beta"), "beta");
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        scan_local(&source_root)
            .unwrap()
            .import_selected(
                &mut library,
                &HashSet::from(["alpha".to_string(), "beta".to_string()]),
                None,
            )
            .unwrap();

        // Linked local skills are excluded; copied skills still report progress.
        for name in ["gamma", "delta"] {
            let path = source_root.join(name);
            write_skill(&path, name);
            library
                .import(
                    &path,
                    SkillRecord {
                        name: name.into(),
                        storage_name: String::new(),
                        description: String::new(),
                        custom_description: None,
                        origin: SkillOrigin::Unknown,
                        update_available: false,
                        group_id: None,
                        last_operated_at: 0,
                    },
                )
                .unwrap();
        }

        let mut reports = Vec::new();
        let count = check_updates_with_progress(&mut library, |progress| {
            reports.push(progress);
        })
        .unwrap();

        assert_eq!(count, 0);
        assert!(
            reports
                .iter()
                .any(|progress| progress.total == 2 && progress.scanned == 0)
        );
        assert_eq!(reports.last().map(|progress| progress.scanned), Some(2));
        assert!(
            reports
                .windows(2)
                .all(|window| window[0].scanned <= window[1].scanned)
        );
        let names = reports
            .iter()
            .filter_map(|progress| progress.current.clone())
            .collect::<HashSet<_>>();
        assert!(names.contains("gamma"));
        assert!(names.contains("delta"));
        assert!(!names.contains("alpha"));
        assert!(!names.contains("beta"));
    }

    #[test]
    fn npx_import_preserves_the_scan_content_and_lock_hash() {
        let temp = tempfile::tempdir().unwrap();
        let scan_temp = tempfile::tempdir().unwrap();
        let scanned_skill = scan_temp.path().join(".agents/skills/alpha");
        fs::create_dir_all(scan_temp.path().join(".agents")).unwrap();
        fs::create_dir_all(&scanned_skill).unwrap();
        fs::write(
            scanned_skill.join("SKILL.md"),
            "---\nname: alpha\ndescription: fresh\n---\nfresh scan\n",
        )
        .unwrap();
        fs::write(
            scan_temp.path().join(".agents/.skill-lock.json"),
            r#"{"skills":{"alpha":{"skillFolderHash":"fresh-hash","sourceUrl":"https://github.com/owner/repository","ref":"stable","skillPath":"skills/custom-path/SKILL.md"}}}"#,
        )
        .unwrap();

        let scan = SkillScan {
            origin: ScanOrigin::Npx {
                repository: "git@github.com:owner/repository.git".into(),
            },
            skills: vec![ScannedSkill {
                name: "alpha".into(),
                description: "fresh".into(),
                path: scanned_skill,
            }],
            git_sources: None,
            _temp: Some(scan_temp),
        };
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        scan.import_selected(&mut library, &HashSet::from(["alpha".to_string()]), None)
            .unwrap();

        let content =
            fs::read_to_string(library.skill_path("alpha").unwrap().join("SKILL.md")).unwrap();
        assert!(content.contains("fresh scan"));
        let record = library.record("alpha").unwrap();
        let locator = library.remote_skill_source(&record).unwrap();
        assert_eq!(locator.repository, "git@github.com:owner/repository.git");
        assert_eq!(locator.reference.as_deref(), Some("stable"));
        assert_eq!(locator.skill_path.as_deref(), Some("skills/custom-path"));
        assert!(matches!(
            library.record("alpha").unwrap().origin,
            SkillOrigin::Npx {
                repository,
                source_hash: Some(hash),
                ..
            } if hash == "fresh-hash" && repository == "git@github.com:owner/repository.git"
        ));
    }

    #[test]
    fn repository_input_preserves_transport() {
        for repository in [
            "git@github.com:owner/repository.git",
            "ssh://git@github.com/owner/repository.git",
            "https://github.com/owner/repository.git",
            "https://gitlab.com/owner/repository/",
        ] {
            assert_eq!(normalize_npx_source(repository).unwrap(), repository);
            assert_eq!(
                normalize_npx_source(&format!("npx skills add {repository}")).unwrap(),
                repository
            );
            assert_eq!(
                remote::RemoteSkill::parse(repository).unwrap().repository,
                repository
            );
        }
    }

    #[test]
    fn lock_metadata_preserves_requested_repository_and_resolved_skill_location() {
        let entry: NpxLockEntry = serde_json::from_value(serde_json::json!({
            "sourceUrl": "https://github.com/owner/repository",
            "ref": "stable",
            "skillPath": "docs/custom-folder/SKILL.md"
        }))
        .unwrap();
        for repository in [
            "git@github.com:owner/repository.git",
            "ssh://git@github.com/owner/repository.git",
            "https://github.com/owner/repository.git",
            "https://github.com/owner/repository/",
        ] {
            let source = entry.remote_source(repository).unwrap();
            assert_eq!(source.repository, repository);
            assert_eq!(source.reference.as_deref(), Some("stable"));
            assert_eq!(source.skill_path.as_deref(), Some("docs/custom-folder"));
        }
        let shorthand = entry.remote_source("owner/repository@main").unwrap();
        assert_eq!(shorthand.repository, "https://github.com/owner/repository");
        assert_eq!(shorthand.reference.as_deref(), Some("stable"));
        let no_lock_location: NpxLockEntry = serde_json::from_str("{}").unwrap();
        let tree = no_lock_location
            .remote_source("https://github.com/owner/repository/tree/main/docs/custom-folder")
            .unwrap();
        assert_eq!(tree.repository, "https://github.com/owner/repository");
        assert_eq!(tree.reference.as_deref(), Some("main"));
        assert_eq!(tree.skill_path.as_deref(), Some("docs/custom-folder"));
    }
}
