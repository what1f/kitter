//! Presets bundle library Skills with an optional AGENTS.md template and keep
//! track of the projects they were installed into, so later edits can be
//! synced without disturbing anything the preset did not put there.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{InstallTarget, SkillSummary, agents::installation_root, config, project};

const AGENTS_MD: &str = "AGENTS.md";

mod files;
pub use files::{
    FileConflict, FileConflictDecision, FileDecisions, FileState, FileStatus, OwnedFile,
    PresetFileConflicts,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Library storage names, in display order.
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetInstallation {
    pub preset: String,
    pub project: PathBuf,
    pub targets: Vec<InstallTarget>,
    /// Skill links this preset created in the project, one per install
    /// location. Links that were already present stay untouched when the
    /// preset is removed.
    #[serde(default)]
    pub owned: Vec<OwnedLink>,
    /// Hash of the AGENTS.md block body last written by Kitter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents_md_hash: Option<String>,
    #[serde(default)]
    pub files: Vec<OwnedFile>,
    #[serde(default)]
    pub created_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub skipped_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedLink {
    pub skill: String,
    pub target: InstallTarget,
}

#[derive(Default, Serialize, Deserialize)]
struct PresetDocument {
    #[serde(default)]
    presets: Vec<Preset>,
    #[serde(default)]
    installations: Vec<PresetInstallation>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentsMdState {
    #[default]
    Current,
    Outdated,
    /// The managed block was edited in the project after Kitter wrote it.
    Modified,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PresetStatus {
    pub missing_skills: Vec<String>,
    pub extra_skills: Vec<String>,
    pub agents_md: AgentsMdState,
    pub files: Vec<FileStatus>,
}

impl PresetStatus {
    pub fn needs_sync(&self) -> bool {
        !self.missing_skills.is_empty()
            || !self.extra_skills.is_empty()
            || self.agents_md != AgentsMdState::Current
            || self
                .files
                .iter()
                .any(|file| file.state != FileState::Current)
    }
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub installed: usize,
    pub removed: usize,
    pub failures: Vec<SkillFailure>,
}

#[derive(Debug)]
pub struct SkillFailure {
    pub skill: String,
    pub error: String,
}

impl std::fmt::Display for SkillFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&t!(
            "errors.item_detail",
            item = self.skill,
            error = self.error
        ))
    }
}

pub struct PresetStore {
    data_dir: PathBuf,
    document: PresetDocument,
}

impl PresetStore {
    pub fn open() -> Result<Self> {
        Self::open_in(config::app_data_dir())
    }

    pub fn open_in(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        let path = data_dir.join("presets.json");
        let mut document: PresetDocument = if path.exists() {
            let bytes = fs::read(&path).with_context(|| {
                t!("errors.presets_read_failed", path = path.display()).to_string()
            })?;
            serde_json::from_slice(&bytes).context(t!("errors.presets_invalid").to_string())?
        } else {
            PresetDocument::default()
        };
        // Older versions stored Windows paths with the `\\?\` prefix.
        for installation in &mut document.installations {
            installation.project = dunce::simplified(&installation.project).to_path_buf();
        }
        Ok(Self { data_dir, document })
    }

    fn save(&self) -> Result<()> {
        config::save_json(&self.data_dir.join("presets.json"), &self.document)
    }

    pub fn presets(&self) -> &[Preset] {
        &self.document.presets
    }

    pub fn preset(&self, id: &str) -> Result<&Preset> {
        self.document
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .ok_or_else(|| crate::user_error!("errors.preset_not_found", name = id).into())
    }

    /// Resolve a user-facing selector: an exact ID first, then a unique name.
    pub fn resolve(&self, selector: &str) -> Result<&Preset> {
        if let Some(preset) = self.document.presets.iter().find(|p| p.id == selector) {
            return Ok(preset);
        }
        let matches = self
            .document
            .presets
            .iter()
            .filter(|preset| preset.name == selector)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [preset] => Ok(preset),
            [] => crate::user_bail!("errors.preset_not_found", name = selector),
            _ => crate::user_bail!("errors.preset_ambiguous"),
        }
    }

    pub fn create(&mut self, name: &str) -> Result<Preset> {
        let name = self.validate_name(name, None)?;
        let id = self.unique_id(&name);
        let preset = Preset {
            id,
            name,
            skills: Vec::new(),
            created_at: now(),
        };
        self.document.presets.push(preset.clone());
        self.save()?;
        Ok(preset)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        let name = self.validate_name(name, Some(id))?;
        self.preset_mut(id)?.name = name;
        self.save()
    }

    /// Delete a preset after removing it from every project it was installed in.
    pub fn delete(&mut self, id: &str, skills: &[SkillSummary]) -> Result<SyncReport> {
        self.preset(id)?;
        let mut report = SyncReport::default();
        let projects = self
            .installations(id)
            .map(|installation| installation.project.clone())
            .collect::<Vec<_>>();
        for project in projects {
            let partial = self.uninstall(id, &project, skills)?;
            report.removed += partial.removed;
            report.failures.extend(partial.failures);
        }
        self.document.presets.retain(|preset| preset.id != id);
        let directory = self.preset_dir(id);
        if directory.exists() {
            fs::remove_dir_all(&directory).with_context(|| {
                t!("errors.delete_failed", path = directory.display()).to_string()
            })?;
        }
        self.save()?;
        Ok(report)
    }

    pub fn add_skills(&mut self, id: &str, storage_names: &[String]) -> Result<()> {
        let preset = self.preset_mut(id)?;
        for storage_name in storage_names {
            if !preset.skills.contains(storage_name) {
                preset.skills.push(storage_name.clone());
            }
        }
        self.save()
    }

    pub fn remove_skills(&mut self, id: &str, storage_names: &[String]) -> Result<()> {
        self.preset_mut(id)?
            .skills
            .retain(|skill| !storage_names.contains(skill));
        self.save()
    }

    /// Drop a Skill that no longer exists in the library from every preset.
    pub fn forget_skill(&mut self, storage_name: &str) -> Result<()> {
        let mut changed = false;
        for preset in &mut self.document.presets {
            let before = preset.skills.len();
            preset.skills.retain(|skill| skill != storage_name);
            changed |= preset.skills.len() != before;
        }
        for installation in &mut self.document.installations {
            let before = installation.owned.len();
            installation.owned.retain(|link| link.skill != storage_name);
            changed |= installation.owned.len() != before;
        }
        if changed { self.save() } else { Ok(()) }
    }

    pub fn agents_md(&self, id: &str) -> Result<String> {
        let path = self.preset_dir(id).join(AGENTS_MD);
        match fs::read_to_string(&path) {
            Ok(content) => Ok(content),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(error) => Err(error)
                .with_context(|| t!("errors.read_failed", path = path.display()).to_string()),
        }
    }

    pub fn set_agents_md(&mut self, id: &str, content: &str) -> Result<()> {
        self.preset(id)?;
        let directory = self.preset_dir(id);
        let path = directory.join(AGENTS_MD);
        if content.trim().is_empty() {
            if path.exists() {
                fs::remove_file(&path)?;
            }
            return Ok(());
        }
        fs::create_dir_all(&directory)?;
        let temp = path.with_extension("md.tmp");
        fs::write(&temp, content)?;
        fs::rename(temp, path)?;
        Ok(())
    }

    pub fn agents_md_path(&self, id: &str) -> PathBuf {
        self.preset_dir(id).join(AGENTS_MD)
    }

    /// Every project that has at least one preset installed.
    pub fn projects(&self) -> Vec<PathBuf> {
        let mut projects = Vec::new();
        for installation in &self.document.installations {
            if !projects.contains(&installation.project) {
                projects.push(installation.project.clone());
            }
        }
        projects
    }

    pub fn installations(&self, id: &str) -> impl Iterator<Item = &PresetInstallation> {
        let id = id.to_string();
        self.document
            .installations
            .iter()
            .filter(move |installation| installation.preset == id)
    }

    pub fn project_installations(
        &self,
        project: &Path,
    ) -> impl Iterator<Item = &PresetInstallation> {
        let project = project_key(project);
        self.document
            .installations
            .iter()
            .filter(move |installation| installation.project == project)
    }

    pub fn installation(&self, id: &str, project: &Path) -> Option<&PresetInstallation> {
        let project = project_key(project);
        let project = project.as_path();
        self.document
            .installations
            .iter()
            .find(|installation| installation.preset == id && installation.project == project)
    }

    /// Install a preset into a project. Installing again adds any new targets
    /// and brings the project up to date with the preset.
    pub fn install(
        &mut self,
        id: &str,
        project: &Path,
        targets: &[InstallTarget],
        skills: &[SkillSummary],
    ) -> Result<SyncReport> {
        self.install_with_files(id, project, targets, skills, &FileDecisions::new())
    }

    pub fn install_with_files(
        &mut self,
        id: &str,
        project: &Path,
        targets: &[InstallTarget],
        skills: &[SkillSummary],
        decisions: &FileDecisions,
    ) -> Result<SyncReport> {
        self.preset(id)?;
        if targets.is_empty() {
            crate::user_bail!("errors.install_location_required");
        }
        if !project.is_dir() {
            crate::user_bail!("errors.project_missing", path = project.display());
        }
        self.check_file_decisions(id, project, decisions)?;
        // Global installs only link skills; instruction files belong to projects.
        if is_home(project) && self.preset(id)?.skills.is_empty() {
            crate::user_bail!("errors.preset_global_without_skills");
        }
        let project = project_key(project);
        let project = project.as_path();
        match self
            .document
            .installations
            .iter_mut()
            .find(|installation| installation.preset == id && installation.project == project)
        {
            Some(installation) => {
                for target in targets {
                    if !installation.targets.contains(target) {
                        installation.targets.push(target.clone());
                    }
                }
            }
            None => self.document.installations.push(PresetInstallation {
                preset: id.to_string(),
                project: project.to_path_buf(),
                targets: targets.to_vec(),
                owned: Vec::new(),
                agents_md_hash: None,
                files: Vec::new(),
                created_dirs: Vec::new(),
                skipped_files: Vec::new(),
            }),
        }
        self.sync_with_files(id, project, skills, true, decisions)
    }

    /// Keep ownership of links created by the reviewed installation plan so
    /// removing the preset later removes exactly those links.
    pub fn install_reviewed(
        &mut self,
        id: &str,
        project: &Path,
        targets: &[InstallTarget],
        skills: &[SkillSummary],
        plan: project::InstallPlan,
    ) -> Result<SyncReport> {
        self.install_reviewed_with_files(id, project, targets, skills, plan, &FileDecisions::new())
    }

    pub fn install_reviewed_with_files(
        &mut self,
        id: &str,
        project: &Path,
        targets: &[InstallTarget],
        skills: &[SkillSummary],
        plan: project::InstallPlan,
        decisions: &FileDecisions,
    ) -> Result<SyncReport> {
        self.check_file_decisions(id, project, decisions)?;
        let preset = self.preset(id)?.clone();
        let mut created = Vec::new();
        let mut changed_skills = HashSet::new();
        for storage in &preset.skills {
            let Some(skill) = find_skill(skills, storage) else {
                continue;
            };
            for target in targets {
                if crate::agents::installation_root(project, target).is_some()
                    && !is_linked(project, target, skill)
                {
                    created.push(OwnedLink {
                        skill: storage.clone(),
                        target: target.clone(),
                    });
                    changed_skills.insert(storage.clone());
                }
            }
        }
        plan.execute()?;
        let mut report = self.install_with_files(id, project, targets, skills, decisions)?;
        let index = self.installation_index(id, &project_key(project)).unwrap();
        let owned = &mut self.document.installations[index].owned;
        for link in created {
            if !owned.contains(&link) {
                owned.push(link);
            }
        }
        self.save()?;
        report.installed = changed_skills.len();
        Ok(report)
    }

    pub fn status(
        &self,
        id: &str,
        project: &Path,
        skills: &[SkillSummary],
    ) -> Result<PresetStatus> {
        let project = project_key(project);
        let project = project.as_path();
        let preset = self.preset(id)?;
        let installation = self.installation(id, project).ok_or_else(|| {
            crate::user_error!("errors.preset_not_installed", path = project.display())
        })?;
        let mut status = PresetStatus::default();
        for storage_name in &preset.skills {
            let Some(skill) = find_skill(skills, storage_name) else {
                continue;
            };
            if !installation
                .targets
                .iter()
                .filter(|target| crate::agents::installation_root(project, target).is_some())
                .all(|target| is_linked(project, target, skill))
            {
                status.missing_skills.push(storage_name.clone());
            }
        }
        for link in &installation.owned {
            if !preset.skills.contains(&link.skill) && !status.extra_skills.contains(&link.skill) {
                status.extra_skills.push(link.skill.clone());
            }
        }
        status.agents_md = self.agents_md_state(id, installation)?;
        status.files = self.file_status(id, project)?;
        Ok(status)
    }

    /// Bring one project in line with the preset. A locally edited AGENTS.md
    /// block is only replaced when `overwrite_modified` is set.
    pub fn sync(
        &mut self,
        id: &str,
        project: &Path,
        skills: &[SkillSummary],
        overwrite_modified: bool,
    ) -> Result<SyncReport> {
        self.sync_with_files(
            id,
            project,
            skills,
            overwrite_modified,
            &FileDecisions::new(),
        )
    }

    pub fn sync_with_files(
        &mut self,
        id: &str,
        project: &Path,
        skills: &[SkillSummary],
        overwrite_modified: bool,
        decisions: &FileDecisions,
    ) -> Result<SyncReport> {
        let project = project_key(project);
        let project = project.as_path();
        let preset = self.preset(id)?.clone();
        let index = self.installation_index(id, project).ok_or_else(|| {
            crate::user_error!("errors.preset_not_installed", path = project.display())
        })?;
        let installation = self.document.installations[index].clone();
        let agents_md_state = self.agents_md_state(id, &installation)?;
        if agents_md_state == AgentsMdState::Modified && !overwrite_modified {
            crate::user_bail!("errors.preset_agents_md_modified");
        }

        self.check_file_decisions(id, project, decisions)?;
        let modified = self
            .file_status(id, project)?
            .into_iter()
            .filter(|f| f.state == FileState::Modified && !f.removed)
            .map(|f| f.path)
            .collect::<Vec<_>>();
        if !modified.is_empty() && !overwrite_modified {
            crate::user_bail!(
                "errors.preset_files_modified",
                paths = modified
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        let mut file_installation = installation.clone();
        self.sync_files(id, &mut file_installation, decisions)?;
        let mut report = SyncReport::default();
        let mut owned = installation.owned.clone();
        for storage_name in &preset.skills {
            let Some(skill) = find_skill(skills, storage_name) else {
                continue;
            };
            let missing = installation
                .targets
                .iter()
                .filter(|target| {
                    crate::agents::installation_root(project, target).is_some()
                        && !is_linked(project, target, skill)
                })
                .cloned()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                continue;
            }
            // A missing link is not anyone's yet, so the preset owns what it creates.
            match project::install_from_path(project, &skill.path, &skill.record.name, &missing) {
                Ok(()) => {
                    report.installed += 1;
                    for target in missing {
                        let link = OwnedLink {
                            skill: storage_name.clone(),
                            target,
                        };
                        if !owned.contains(&link) {
                            owned.push(link);
                        }
                    }
                }
                Err(error) => report.failures.push(SkillFailure {
                    skill: skill.record.name.clone(),
                    error: format!("{error:#}"),
                }),
            }
        }

        let (extra, kept): (Vec<_>, Vec<_>) = owned
            .into_iter()
            .partition(|link| !preset.skills.contains(&link.skill));
        let owned = kept;
        self.release_links(project, id, &extra, skills, &mut report);

        let content = self.agents_md(id)?;
        let hash = if is_home(project) {
            None
        } else {
            write_agents_md_block(project, id, &content)?
        };
        let installation = &mut self.document.installations[index];
        installation.owned = owned;
        installation.agents_md_hash = hash;
        installation.files = file_installation.files;
        installation.created_dirs = file_installation.created_dirs;
        installation.skipped_files = file_installation.skipped_files;
        self.save()?;
        Ok(report)
    }

    /// Remove a preset from a project: its AGENTS.md block and the Skills it
    /// installed that no other preset in the project still needs.
    pub fn uninstall(
        &mut self,
        id: &str,
        project: &Path,
        skills: &[SkillSummary],
    ) -> Result<SyncReport> {
        let project = project_key(project);
        let project = project.as_path();
        let index = self.installation_index(id, project).ok_or_else(|| {
            crate::user_error!("errors.preset_not_installed", path = project.display())
        })?;
        let installation = self.document.installations[index].clone();
        files::release_files(project, &installation.files)?;
        let mut dirs = installation.created_dirs.clone();
        files::cleanup_dirs(project, &mut dirs);
        self.document.installations.remove(index);
        let mut report = SyncReport::default();
        self.release_links(project, id, &installation.owned, skills, &mut report);
        if !is_home(project) {
            remove_agents_md_block(project, id)?;
        }
        self.save()?;
        Ok(report)
    }

    fn release_links(
        &mut self,
        project: &Path,
        id: &str,
        links: &[OwnedLink],
        skills: &[SkillSummary],
        report: &mut SyncReport,
    ) {
        let mut removed = HashSet::new();
        for link in links {
            if self.release_link(project, id, link, skills, report) {
                removed.insert(&link.skill);
            }
        }
        report.removed += removed.len();
    }

    /// Returns whether the link was removed from the project.
    fn release_link(
        &mut self,
        project: &Path,
        id: &str,
        link: &OwnedLink,
        skills: &[SkillSummary],
        report: &mut SyncReport,
    ) -> bool {
        // Another preset in the same project still wants this Skill at this
        // location: hand the link over instead of removing it.
        let heir =
            self.document.installations.iter().position(|other| {
                other.project == project
                    && other.preset != id
                    && other.targets.contains(&link.target)
                    && self.document.presets.iter().any(|preset| {
                        preset.id == other.preset && preset.skills.contains(&link.skill)
                    })
            });
        if let Some(heir) = heir {
            let owned = &mut self.document.installations[heir].owned;
            if !owned.contains(link) {
                owned.push(link.clone());
            }
            return false;
        }
        let Some(skill) = find_skill(skills, &link.skill) else {
            return false;
        };
        match project::uninstall_from_path(
            project,
            &skill.path,
            &skill.record.name,
            std::slice::from_ref(&link.target),
        ) {
            Ok(()) => true,
            Err(error) => {
                report.failures.push(SkillFailure {
                    skill: skill.record.name.clone(),
                    error: format!("{error:#}"),
                });
                false
            }
        }
    }

    fn agents_md_state(
        &self,
        id: &str,
        installation: &PresetInstallation,
    ) -> Result<AgentsMdState> {
        if is_home(&installation.project) {
            return Ok(AgentsMdState::Current);
        }
        let desired = normalized_body(&self.agents_md(id)?);
        let file = read_optional(&installation.project.join(AGENTS_MD))?;
        let current = file
            .as_deref()
            .and_then(|text| find_block(text, id))
            .map(|block| block.body);
        if let (Some(current), Some(recorded)) = (&current, &installation.agents_md_hash)
            && &content_hash(current) != recorded
        {
            return Ok(AgentsMdState::Modified);
        }
        let up_to_date = match (&desired, &current) {
            (None, None) => true,
            (Some(desired), Some(current)) => desired == current,
            _ => false,
        };
        Ok(if up_to_date {
            AgentsMdState::Current
        } else {
            AgentsMdState::Outdated
        })
    }

    fn installation_index(&self, id: &str, project: &Path) -> Option<usize> {
        self.document
            .installations
            .iter()
            .position(|installation| installation.preset == id && installation.project == project)
    }

    fn preset_mut(&mut self, id: &str) -> Result<&mut Preset> {
        self.document
            .presets
            .iter_mut()
            .find(|preset| preset.id == id)
            .ok_or_else(|| crate::user_error!("errors.preset_not_found", name = id).into())
    }

    fn preset_dir(&self, id: &str) -> PathBuf {
        self.data_dir.join("presets").join(id)
    }

    fn validate_name(&self, name: &str, current: Option<&str>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            crate::user_bail!("errors.preset_name_required");
        }
        if self
            .document
            .presets
            .iter()
            .any(|preset| preset.name == name && Some(preset.id.as_str()) != current)
        {
            crate::user_bail!("errors.preset_duplicate");
        }
        Ok(name.to_string())
    }

    fn unique_id(&self, name: &str) -> String {
        let slug = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if slug.is_empty() {
            "preset".to_string()
        } else {
            slug
        };
        let taken = |id: &str| self.document.presets.iter().any(|preset| preset.id == id);
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|index| format!("{base}-{index}"))
            .find(|id| !taken(id))
            .unwrap_or(base)
    }
}

fn find_skill<'a>(skills: &'a [SkillSummary], storage_name: &str) -> Option<&'a SkillSummary> {
    skills
        .iter()
        .find(|skill| skill.record.storage_name == storage_name)
}

fn is_linked(project: &Path, target: &InstallTarget, skill: &SkillSummary) -> bool {
    let Some(root) = installation_root(project, target) else {
        return false;
    };
    let link = root.join(&skill.record.name);
    match (link.canonicalize(), skill.path.canonicalize()) {
        (Ok(link), Ok(source)) => link == source,
        _ => false,
    }
}

/// Projects are matched by their resolved path so the CLI and the desktop app
/// agree on the same folder however it was chosen.
fn project_key(project: &Path) -> PathBuf {
    dunce::canonicalize(project).unwrap_or_else(|_| project.to_path_buf())
}

fn is_home(project: &Path) -> bool {
    dirs::home_dir().is_some_and(|home| project_key(&home) == project_key(project))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| t!("errors.read_failed", path = path.display()).to_string())
        }
    }
}

fn write_text(path: &Path, content: &str) -> Result<()> {
    let temp = path.with_extension("md.kitter-tmp");
    let context = || t!("errors.write_failed", path = path.display()).to_string();
    fs::write(&temp, content).with_context(context)?;
    fs::rename(&temp, path).with_context(context)
}

/// FNV-1a: stable across Rust releases, unlike `DefaultHasher`.
fn content_hash(content: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn normalized_body(content: &str) -> Option<String> {
    let body = content.trim_matches(|c| c == '\n' || c == '\r').trim_end();
    (!body.is_empty()).then(|| body.replace("\r\n", "\n"))
}

fn start_marker(id: &str) -> String {
    format!("<!-- kitter:preset:{id} -->")
}

fn end_marker(id: &str) -> String {
    format!("<!-- /kitter:preset:{id} -->")
}

struct Block {
    start: usize,
    end: usize,
    body: String,
}

fn find_block(text: &str, id: &str) -> Option<Block> {
    let start_marker = start_marker(id);
    let end_marker = end_marker(id);
    let start = text.find(&start_marker)?;
    let body_start = start + start_marker.len();
    let end_offset = text[body_start..].find(&end_marker)?;
    let body_end = body_start + end_offset;
    let mut end = body_end + end_marker.len();
    if text[end..].starts_with("\r\n") {
        end += 2;
    } else if text[end..].starts_with('\n') {
        end += 1;
    }
    let body = normalized_body(&text[body_start..body_end]).unwrap_or_default();
    Some(Block { start, end, body })
}

fn render_block(id: &str, body: &str) -> String {
    format!("{}\n{}\n{}\n", start_marker(id), body, end_marker(id))
}

/// Write the preset's block into the project's AGENTS.md and return the hash of
/// what was written, or `None` when the preset has no AGENTS.md content.
fn write_agents_md_block(project: &Path, id: &str, content: &str) -> Result<Option<String>> {
    let Some(body) = normalized_body(content) else {
        remove_agents_md_block(project, id)?;
        return Ok(None);
    };
    let path = project.join(AGENTS_MD);
    let existing = read_optional(&path)?.unwrap_or_default();
    let block = render_block(id, &body);
    let updated = match find_block(&existing, id) {
        Some(found) => format!(
            "{}{}{}",
            &existing[..found.start],
            block,
            &existing[found.end..]
        ),
        None if existing.trim().is_empty() => block,
        None => format!("{}\n\n{}", existing.trim_end(), block),
    };
    if updated != existing {
        write_text(&path, &updated)?;
    }
    Ok(Some(content_hash(&body)))
}

fn remove_agents_md_block(project: &Path, id: &str) -> Result<()> {
    let path = project.join(AGENTS_MD);
    let Some(existing) = read_optional(&path)? else {
        return Ok(());
    };
    let Some(found) = find_block(&existing, id) else {
        return Ok(());
    };
    let before = existing[..found.start].trim_end();
    let after = existing[found.end..].trim_start_matches(['\n', '\r']);
    let updated = match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    };
    if updated.trim().is_empty() {
        fs::remove_file(&path)
            .with_context(|| t!("errors.delete_failed", path = path.display()).to_string())?;
    } else {
        write_text(&path, &updated)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SkillOrigin, SkillRecord};

    struct Fixture {
        _temp: tempfile::TempDir,
        data: PathBuf,
        project: PathBuf,
        skills: Vec<SkillSummary>,
    }

    impl Fixture {
        fn new(names: &[&str]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let data = temp.path().join("data");
            let library = data.join("skills");
            let project = temp.path().join("project");
            fs::create_dir_all(&project).unwrap();
            let skills = names
                .iter()
                .map(|name| {
                    let path = library.join(name);
                    fs::create_dir_all(&path).unwrap();
                    fs::write(path.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
                    SkillSummary {
                        original_description: String::new(),
                        version: None,
                        record: SkillRecord {
                            name: name.to_string(),
                            storage_name: name.to_string(),
                            description: String::new(),
                            custom_description: None,
                            origin: SkillOrigin::Unknown,
                            update_available: false,
                            group_id: None,
                            last_operated_at: 0,
                        },
                        path,
                        installed_projects: 0,
                        manual_only: false,
                    }
                })
                .collect();
            Self {
                _temp: temp,
                data,
                project,
                skills,
            }
        }

        fn store(&self) -> PresetStore {
            PresetStore::open_in(&self.data).unwrap()
        }

        fn link(&self, name: &str) -> PathBuf {
            self.project.join(".agents/skills").join(name)
        }

        fn agents_md(&self) -> Option<String> {
            fs::read_to_string(self.project.join(AGENTS_MD)).ok()
        }
    }

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn universal() -> Vec<InstallTarget> {
        vec![InstallTarget::universal()]
    }

    #[test]
    fn preset_file_crud_validates_paths_and_text() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        for path in [
            "",
            "../secret",
            "docs/../secret",
            "/tmp/secret",
            "AGENTS.md",
            "./AGENTS.md",
            "docs/AGENTS.md",
            "C:/secret",
            "docs\\..\\secret",
        ] {
            assert!(
                store.write_file(&id, Path::new(path), "text").is_err(),
                "{path}"
            );
            assert!(store.read_file(&id, Path::new(path)).is_err(), "{path}");
            assert!(store.delete_file(&id, Path::new(path)).is_err(), "{path}");
        }
        for bytes in [&[0xff_u8][..], b"binary\0content", b"bad\x01text"] {
            assert!(store.write_file(&id, Path::new("bad"), bytes).is_err());
        }
        let path = Path::new("docs/conventions.md");
        store.write_file(&id, path, "Hello 世界\n").unwrap();
        assert_eq!(store.files(&id).unwrap(), [path]);
        assert_eq!(store.read_file(&id, path).unwrap(), "Hello 世界\n");
        store.write_file(&id, path, "updated").unwrap();
        let moved = Path::new(".cursor/rules/swift.mdc");
        store.rename_file(&id, path, moved).unwrap();
        assert_eq!(store.read_file(&id, moved).unwrap(), "updated");
        store.write_file(&id, path, "keep").unwrap();
        assert!(store.rename_file(&id, moved, path).is_err());
        store.delete_file(&id, path).unwrap();
        store.delete_file(&id, moved).unwrap();
        assert!(store.files(&id).unwrap().is_empty());
    }

    #[test]
    fn file_installation_conflicts_are_preflighted_and_decided_per_path() {
        let fixture = Fixture::new(&["alpha"]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        store.add_skills(&id, &names(&["alpha"])).unwrap();
        for path in ["overwrite", "skip", "identical", "new/deep/file"] {
            store.write_file(&id, Path::new(path), "desired").unwrap();
        }
        fs::write(fixture.project.join("overwrite"), "old").unwrap();
        fs::write(fixture.project.join("skip"), "mine").unwrap();
        fs::write(fixture.project.join("identical"), "desired").unwrap();
        let error = store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<PresetFileConflicts>().unwrap().paths,
            [Path::new("overwrite"), Path::new("skip")]
        );
        assert!(store.installation(&id, &fixture.project).is_none());
        assert!(!fixture.link("alpha").exists());
        assert!(!fixture.project.join("new").exists());
        let mut decisions = FileDecisions::new();
        decisions.insert("overwrite".into(), FileConflictDecision::Overwrite);
        decisions.insert("skip".into(), FileConflictDecision::Skip);
        store
            .install_with_files(
                &id,
                &fixture.project,
                &universal(),
                &fixture.skills,
                &decisions,
            )
            .unwrap();
        let installation = store.installation(&id, &fixture.project).unwrap();
        assert_eq!(installation.files.len(), 3);
        assert!(
            !installation
                .files
                .iter()
                .any(|f| f.path == Path::new("skip"))
        );
        assert_eq!(
            fs::read_to_string(fixture.project.join("overwrite")).unwrap(),
            "desired"
        );
        let mut store = fixture.store();
        store
            .write_file(&id, Path::new("skip"), "new desired")
            .unwrap();
        store
            .sync(&id, &fixture.project, &fixture.skills, false)
            .unwrap();
        assert_eq!(
            fs::read_to_string(fixture.project.join("skip")).unwrap(),
            "mine"
        );
        store
            .uninstall(&id, &fixture.project, &fixture.skills)
            .unwrap();
        assert!(!fixture.project.join("identical").exists());
        assert!(!fixture.project.join("new").exists());
        assert!(fixture.project.join("skip").exists());
    }

    #[test]
    fn file_sync_requires_confirmation_and_preserves_edits_on_removal() {
        let fixture = Fixture::new(&[]);
        fs::create_dir(fixture.project.join("existing")).unwrap();
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        for path in ["existing/keep", "created/deep/clean", "created/edited"] {
            store.write_file(&id, Path::new(path), "v1").unwrap();
        }
        store
            .install(&id, &fixture.project, &universal(), &[])
            .unwrap();
        assert!(
            !store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .needs_sync()
        );
        store
            .write_file(&id, Path::new("created/deep/clean"), "v2")
            .unwrap();
        assert!(
            store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .needs_sync()
        );
        store.sync(&id, &fixture.project, &[], false).unwrap();
        assert_eq!(
            fs::read_to_string(fixture.project.join("created/deep/clean")).unwrap(),
            "v2"
        );
        fs::write(fixture.project.join("created/deep/clean"), "mine").unwrap();
        assert_eq!(
            store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .files
                .iter()
                .find(|f| f.path == Path::new("created/deep/clean"))
                .unwrap()
                .state,
            FileState::Modified
        );
        assert!(store.sync(&id, &fixture.project, &[], false).is_err());
        assert_eq!(
            fs::read_to_string(fixture.project.join("created/deep/clean")).unwrap(),
            "mine"
        );
        store.sync(&id, &fixture.project, &[], true).unwrap();
        fs::write(fixture.project.join("created/edited"), "my edits").unwrap();
        store.delete_file(&id, Path::new("created/edited")).unwrap();
        store
            .delete_file(&id, Path::new("created/deep/clean"))
            .unwrap();
        let status = store.status(&id, &fixture.project, &[]).unwrap();
        assert!(
            status
                .files
                .iter()
                .any(|f| f.removed && f.state == FileState::Modified)
        );
        store.sync(&id, &fixture.project, &[], false).unwrap();
        assert!(!fixture.project.join("created/deep").exists());
        assert!(fixture.project.join("created/edited").exists());
        assert!(
            !store
                .installation(&id, &fixture.project)
                .unwrap()
                .files
                .iter()
                .any(|f| f.path == Path::new("created/edited"))
        );
        assert!(
            !store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .needs_sync()
        );
        // Local changes to a still-managed file survive uninstall too.
        fs::write(fixture.project.join("existing/keep"), "keep my changes").unwrap();
        store.uninstall(&id, &fixture.project, &[]).unwrap();
        assert!(fixture.project.join("existing/keep").exists());
        assert!(fixture.project.join("created/edited").exists());
        assert!(fixture.project.join("existing").is_dir());
    }

    #[test]
    fn new_files_conflict_during_sync_and_unresolved_conflicts_do_not_update_other_files() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        store.write_file(&id, Path::new("old"), "v1").unwrap();
        store
            .install(&id, &fixture.project, &universal(), &[])
            .unwrap();
        store.write_file(&id, Path::new("old"), "v2").unwrap();
        store.write_file(&id, Path::new("new"), "desired").unwrap();
        fs::write(fixture.project.join("new"), "mine").unwrap();
        assert_eq!(
            store.file_conflicts(&id, &fixture.project).unwrap(),
            [FileConflict { path: "new".into() }]
        );
        assert!(
            store
                .sync(&id, &fixture.project, &[], true)
                .unwrap_err()
                .downcast_ref::<PresetFileConflicts>()
                .is_some()
        );
        assert_eq!(
            fs::read_to_string(fixture.project.join("old")).unwrap(),
            "v1"
        );
        let decisions = [(PathBuf::from("new"), FileConflictDecision::Overwrite)]
            .into_iter()
            .collect();
        store
            .sync_with_files(&id, &fixture.project, &[], false, &decisions)
            .unwrap();
        assert_eq!(
            fs::read_to_string(fixture.project.join("new")).unwrap(),
            "desired"
        );
    }

    #[test]
    fn old_installations_default_to_no_managed_files() {
        let installation: PresetInstallation = serde_json::from_value(serde_json::json!({
            "preset": "old", "project": "/project", "targets": ["universal"]
        }))
        .unwrap();
        assert!(installation.files.is_empty());
        assert!(installation.created_dirs.is_empty());
        assert!(installation.skipped_files.is_empty());
    }

    #[test]
    fn identical_new_files_need_sync_and_local_deletions_need_confirmation() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        store
            .install(&id, &fixture.project, &universal(), &[])
            .unwrap();
        store.write_file(&id, Path::new("same"), "same").unwrap();
        fs::write(fixture.project.join("same"), "same").unwrap();
        assert!(
            store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .needs_sync()
        );
        assert!(
            store
                .file_conflicts(&id, &fixture.project)
                .unwrap()
                .is_empty()
        );
        store.sync(&id, &fixture.project, &[], false).unwrap();
        assert!(
            !store
                .status(&id, &fixture.project, &[])
                .unwrap()
                .needs_sync()
        );
        fs::remove_file(fixture.project.join("same")).unwrap();
        assert_eq!(
            store.status(&id, &fixture.project, &[]).unwrap().files[0].state,
            FileState::Modified
        );
        assert!(store.sync(&id, &fixture.project, &[], false).is_err());
        assert!(!fixture.project.join("same").exists());
        store.sync(&id, &fixture.project, &[], true).unwrap();
        assert!(fixture.project.join("same").exists());
    }

    #[test]
    fn reviewed_install_checks_files_before_executing_skill_plan() {
        let fixture = Fixture::new(&["alpha"]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        store.add_skills(&id, &names(&["alpha"])).unwrap();
        store
            .write_file(&id, Path::new("conflict"), "desired")
            .unwrap();
        fs::write(fixture.project.join("conflict"), "mine").unwrap();
        let mut plan = project::InstallPlan::default();
        plan.add(
            &fixture.project,
            &fixture.skills[0].path,
            "alpha",
            &universal(),
        )
        .unwrap();
        assert!(
            store
                .install_reviewed(&id, &fixture.project, &universal(), &fixture.skills, plan)
                .is_err()
        );
        assert!(!fixture.link("alpha").exists());
    }

    #[cfg(unix)]
    #[test]
    fn preset_files_reject_symlinks_in_storage_and_projects() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("files").unwrap().id;
        store
            .write_file(&id, Path::new("docs/file"), "desired")
            .unwrap();
        let external = fixture.data.join("external");
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("file"), "keep").unwrap();
        symlink(&external, fixture.project.join("docs")).unwrap();
        assert!(
            store
                .install(&id, &fixture.project, &universal(), &[])
                .is_err()
        );
        assert_eq!(fs::read_to_string(external.join("file")).unwrap(), "keep");
        symlink(&external, store.preset_dir(&id).join("files/escape")).unwrap();
        assert!(store.files(&id).is_err());
        assert!(
            store
                .write_file(&id, Path::new("escape/file"), "bad")
                .is_err()
        );
    }

    #[test]
    fn ids_are_slugs_and_names_must_be_unique() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        assert_eq!(store.create("Frontend App").unwrap().id, "frontend-app");
        assert_eq!(store.create("frontend app").unwrap().id, "frontend-app-2");
        assert_eq!(store.create("前端").unwrap().id, "preset");
        assert!(store.create(" Frontend App ").is_err());
        assert!(store.create("  ").is_err());

        let reopened = fixture.store();
        assert_eq!(reopened.presets().len(), 3);
        assert_eq!(reopened.resolve("前端").unwrap().id, "preset");
    }

    #[test]
    fn install_links_skills_and_appends_agents_md_after_existing_content() {
        let fixture = Fixture::new(&["alpha", "beta"]);
        fs::write(fixture.project.join(AGENTS_MD), "# Project\n").unwrap();
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.add_skills(&id, &names(&["alpha", "beta"])).unwrap();
        store.set_agents_md(&id, "Use pnpm.\n").unwrap();

        let report = store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        assert_eq!(report.installed, 2);
        assert!(report.failures.is_empty());
        assert!(fixture.link("alpha").join("SKILL.md").is_file());
        assert_eq!(
            fixture.agents_md().unwrap(),
            "# Project\n\n<!-- kitter:preset:web -->\nUse pnpm.\n<!-- /kitter:preset:web -->\n"
        );
        assert!(
            !store
                .status(&id, &fixture.project, &fixture.skills)
                .unwrap()
                .needs_sync()
        );
    }

    #[test]
    fn sync_applies_preset_edits_and_keeps_pre_existing_skills() {
        let fixture = Fixture::new(&["alpha", "beta", "gamma"]);
        project::install_from_path(
            &fixture.project,
            &fixture.skills[0].path,
            "alpha",
            &universal(),
        )
        .unwrap();
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.add_skills(&id, &names(&["alpha", "beta"])).unwrap();
        store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap();

        store
            .remove_skills(&id, &names(&["alpha", "beta"]))
            .unwrap();
        store.add_skills(&id, &names(&["gamma"])).unwrap();
        store.set_agents_md(&id, "New rules").unwrap();
        let status = store
            .status(&id, &fixture.project, &fixture.skills)
            .unwrap();
        assert_eq!(status.missing_skills, names(&["gamma"]));
        assert_eq!(status.extra_skills, names(&["beta"]));
        assert_eq!(status.agents_md, AgentsMdState::Outdated);

        store
            .sync(&id, &fixture.project, &fixture.skills, false)
            .unwrap();
        // alpha was there before the preset, so it is never removed by it.
        assert!(fixture.link("alpha").exists());
        assert!(fixture.link("beta").symlink_metadata().is_err());
        assert!(fixture.link("gamma").exists());
        assert!(fixture.agents_md().unwrap().contains("New rules"));
        assert!(
            !store
                .status(&id, &fixture.project, &fixture.skills)
                .unwrap()
                .needs_sync()
        );
    }

    #[test]
    fn locally_edited_block_requires_explicit_overwrite() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.set_agents_md(&id, "Original").unwrap();
        store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        let path = fixture.project.join(AGENTS_MD);
        let edited = fs::read_to_string(&path)
            .unwrap()
            .replace("Original", "Tweaked");
        fs::write(&path, edited).unwrap();

        let status = store
            .status(&id, &fixture.project, &fixture.skills)
            .unwrap();
        assert_eq!(status.agents_md, AgentsMdState::Modified);
        assert!(
            store
                .sync(&id, &fixture.project, &fixture.skills, false)
                .is_err()
        );
        assert!(fixture.agents_md().unwrap().contains("Tweaked"));

        store
            .sync(&id, &fixture.project, &fixture.skills, true)
            .unwrap();
        assert!(fixture.agents_md().unwrap().contains("Original"));
    }

    #[test]
    fn uninstall_hands_shared_skills_to_remaining_presets() {
        let fixture = Fixture::new(&["alpha", "beta"]);
        fs::write(fixture.project.join(AGENTS_MD), "# Project\n").unwrap();
        let mut store = fixture.store();
        let web = store.create("web").unwrap().id;
        let rust = store.create("rust").unwrap().id;
        store.add_skills(&web, &names(&["alpha", "beta"])).unwrap();
        store.add_skills(&rust, &names(&["alpha"])).unwrap();
        store.set_agents_md(&web, "Web").unwrap();
        store.set_agents_md(&rust, "Rust").unwrap();
        store
            .install(&web, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        store
            .install(&rust, &fixture.project, &universal(), &fixture.skills)
            .unwrap();

        store
            .uninstall(&web, &fixture.project, &fixture.skills)
            .unwrap();
        assert!(fixture.link("alpha").exists());
        assert!(fixture.link("beta").symlink_metadata().is_err());
        assert_eq!(
            fixture.agents_md().unwrap(),
            "# Project\n\n<!-- kitter:preset:rust -->\nRust\n<!-- /kitter:preset:rust -->\n"
        );

        store
            .uninstall(&rust, &fixture.project, &fixture.skills)
            .unwrap();
        assert!(fixture.link("alpha").symlink_metadata().is_err());
        assert_eq!(fixture.agents_md().unwrap(), "# Project\n");
    }

    #[test]
    fn handover_only_covers_locations_the_remaining_preset_uses() {
        let fixture = Fixture::new(&["alpha"]);
        let mut store = fixture.store();
        let web = store.create("web").unwrap().id;
        let rust = store.create("rust").unwrap().id;
        store.add_skills(&web, &names(&["alpha"])).unwrap();
        store.add_skills(&rust, &names(&["alpha"])).unwrap();
        let both = &[
            InstallTarget::universal(),
            InstallTarget::from("claude-code"),
        ];
        store
            .install(&web, &fixture.project, both, &fixture.skills)
            .unwrap();
        store
            .install(&rust, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        let claude_link =
            crate::agents::installation_root(&fixture.project, &InstallTarget::from("claude-code"))
                .unwrap()
                .join("alpha");
        assert!(claude_link.exists());

        store
            .uninstall(&web, &fixture.project, &fixture.skills)
            .unwrap();
        assert!(fixture.link("alpha").exists());
        assert!(claude_link.symlink_metadata().is_err());

        store
            .uninstall(&rust, &fixture.project, &fixture.skills)
            .unwrap();
        assert!(fixture.link("alpha").symlink_metadata().is_err());
    }

    #[test]
    fn agents_md_created_by_a_preset_is_removed_with_it() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.set_agents_md(&id, "Rules").unwrap();
        store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        assert!(fixture.agents_md().is_some());

        store.delete(&id, &fixture.skills).unwrap();
        assert!(fixture.agents_md().is_none());
        assert!(store.presets().is_empty());
        assert!(!store.agents_md_path(&id).exists());
    }

    #[test]
    fn missing_block_is_written_back_on_sync() {
        let fixture = Fixture::new(&[]);
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.set_agents_md(&id, "Rules").unwrap();
        store
            .install(&id, &fixture.project, &universal(), &fixture.skills)
            .unwrap();
        fs::write(fixture.project.join(AGENTS_MD), "# Rewritten\n").unwrap();

        let status = store
            .status(&id, &fixture.project, &fixture.skills)
            .unwrap();
        assert_eq!(status.agents_md, AgentsMdState::Outdated);
        store
            .sync(&id, &fixture.project, &fixture.skills, false)
            .unwrap();
        assert!(
            fixture
                .agents_md()
                .unwrap()
                .starts_with("# Rewritten\n\n<!--")
        );
    }

    #[test]
    fn forgetting_a_skill_removes_it_from_presets() {
        let fixture = Fixture::new(&["alpha"]);
        let mut store = fixture.store();
        let id = store.create("web").unwrap().id;
        store.add_skills(&id, &names(&["alpha"])).unwrap();
        store.forget_skill("alpha").unwrap();
        assert!(fixture.store().preset(&id).unwrap().skills.is_empty());
    }
    #[test]
    fn legacy_and_unknown_targets_round_trip_in_preset_state() {
        let state = serde_json::json!({
            "installations": [{
                "preset": "example", "project": "/project",
                "targets": ["claude_code", "open_code", "missing-plugin"],
                "owned": [{ "skill": "example", "target": "claude_code" }]
            }]
        });
        let document: PresetDocument = serde_json::from_value(state).unwrap();
        let output = serde_json::to_value(&document).unwrap();
        assert_eq!(
            output["installations"][0]["targets"],
            serde_json::json!(["claude-code", "opencode", "missing-plugin"])
        );
        assert_eq!(
            output["installations"][0]["owned"][0]["target"],
            "claude-code"
        );
        let restored: PresetDocument = serde_json::from_value(output).unwrap();
        assert_eq!(document.installations, restored.installations);
    }

    #[test]
    fn missing_plugin_targets_are_preserved_and_skipped_on_sync() {
        let fixture = Fixture::new(&["example"]);
        let mut store = fixture.store();
        let id = store.create("Example").unwrap().id;
        store.add_skills(&id, &names(&["example"])).unwrap();
        let targets = vec![InstallTarget::from("missing-plugin")];
        let report = store
            .install(&id, &fixture.project, &targets, &fixture.skills)
            .unwrap();
        assert!(report.failures.is_empty());
        assert_eq!(
            store.installation(&id, &fixture.project).unwrap().targets,
            targets
        );
        assert!(
            store
                .status(&id, &fixture.project, &fixture.skills)
                .unwrap()
                .missing_skills
                .is_empty()
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    }
}
