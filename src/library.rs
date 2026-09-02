use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{
    AppConfig, SkillGroup, SkillOrigin, SkillRecord, SkillSource, SkillSourceRecord, SkillSummary,
    config,
};

pub const KITTER_SKILL_STORAGE: &str = "_kitter-builtin";
const KITTER_SKILL_NAME: &str = "kitter";
const KITTER_SKILL_MD: &str = include_str!("../resources/skills/kitter/SKILL.md");
const KITTER_OPENAI_YAML: &str = include_str!("../resources/skills/kitter/agents/openai.yaml");
const KITTER_INSTALL_CLI_MD: &str =
    include_str!("../resources/skills/kitter/references/install-cli.md");

#[derive(Clone, Default, Serialize, Deserialize)]
struct Registry {
    #[serde(default)]
    skills: HashMap<String, SkillRecord>,
    #[serde(default)]
    sources: HashMap<String, SkillSourceRecord>,
    #[serde(default)]
    groups: Vec<SkillGroup>,
    #[serde(default)]
    source_groups_migrated: bool,
    #[serde(default)]
    adopted_sources: HashMap<String, AdoptedSource>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AdoptedSource {
    source: PathBuf,
    references: Vec<crate::adoption::SkillReference>,
    #[serde(default)]
    previous_library: Option<PathBuf>,
}

pub struct SkillLibrary {
    pub config: AppConfig,
    registry: Registry,
    data_dir: PathBuf,
}

impl SkillLibrary {
    pub fn open() -> Result<Self> {
        Self::open_in(config::app_data_dir())
    }

    pub fn open_in(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        let config = AppConfig::load_from(&data_dir)?;
        fs::create_dir_all(&config.library_dir)?;
        ensure_builtin_skill(&config.library_dir)?;
        let path = data_dir.join("registry.json");
        let mut registry: Registry = if path.exists() {
            serde_json::from_slice(&fs::read(path)?)?
        } else {
            Registry::default()
        };
        let mut changed = false;
        let builtin = registry
            .skills
            .entry(KITTER_SKILL_STORAGE.to_string())
            .or_insert_with(|| {
                changed = true;
                SkillRecord {
                    name: KITTER_SKILL_NAME.to_string(),
                    storage_name: KITTER_SKILL_STORAGE.to_string(),
                    description: String::new(),
                    origin: SkillOrigin::Builtin,
                    update_available: false,
                    group_id: None,
                    last_operated_at: u64::MAX,
                }
            });
        if builtin.name != KITTER_SKILL_NAME
            || builtin.storage_name != KITTER_SKILL_STORAGE
            || !builtin.origin.is_builtin()
            || builtin.update_available
            || builtin.group_id.is_some()
            || builtin.last_operated_at != u64::MAX
        {
            builtin.name = KITTER_SKILL_NAME.to_string();
            builtin.storage_name = KITTER_SKILL_STORAGE.to_string();
            builtin.origin = SkillOrigin::Builtin;
            builtin.update_available = false;
            builtin.group_id = None;
            builtin.last_operated_at = u64::MAX;
            changed = true;
        }
        for (storage_name, record) in registry.skills.iter_mut() {
            if record.storage_name.is_empty() {
                record.storage_name = storage_name.clone();
                changed = true;
            }
        }
        for record in registry.skills.values() {
            let source = record.origin.source();
            let source_record = registry.sources.entry(source.key()).or_insert_with(|| {
                changed = true;
                SkillSourceRecord {
                    source: source.clone(),
                    discovered_skills: Vec::new(),
                    added_skills: Vec::new(),
                }
            });
            if !source_record.discovered_skills.contains(&record.name) {
                source_record.discovered_skills.push(record.name.clone());
                changed = true;
            }
            if !source_record.added_skills.contains(&record.name) {
                source_record.added_skills.push(record.name.clone());
                changed = true;
            }
        }
        if !registry.source_groups_migrated {
            if registry.groups.is_empty() {
                let mut source_skills = BTreeMap::<String, (SkillSource, Vec<String>)>::new();
                for (storage_name, record) in &registry.skills {
                    let source = record.origin.source();
                    let source_key = source.key();
                    source_skills
                        .entry(source_key)
                        .or_insert_with(|| (source.clone(), Vec::new()))
                        .1
                        .push(storage_name.clone());
                }
                let mut used_names = HashSet::new();
                for (source_key, (source, storage_names)) in source_skills {
                    if source_key == "unknown" || storage_names.len() < 2 {
                        continue;
                    }
                    let name = unique_group_name(&source.label(), &mut used_names);
                    let id = migrated_group_id(&source_key, &registry.groups);
                    registry.groups.push(SkillGroup {
                        id: id.clone(),
                        name,
                        created_at: operation_stamp(),
                    });
                    for storage_name in storage_names {
                        if let Some(record) = registry.skills.get_mut(&storage_name) {
                            record.group_id = Some(id.clone());
                        }
                    }
                }
            }
            registry.source_groups_migrated = true;
            changed = true;
        }
        let group_ids = registry
            .groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<HashSet<_>>();
        for record in registry.skills.values_mut() {
            if record
                .group_id
                .as_deref()
                .is_some_and(|group_id| !group_ids.contains(group_id))
            {
                record.group_id = None;
                changed = true;
            }
        }
        if changed {
            for source in registry.sources.values_mut() {
                source.discovered_skills.sort();
                source.added_skills.sort();
            }
        }
        let library = Self {
            config,
            registry,
            data_dir,
        };
        if changed {
            library.save()?;
        }
        Ok(library)
    }

    pub fn save(&self) -> Result<()> {
        config::save_json(&self.data_dir.join("config.json"), &self.config)?;
        config::save_json(&self.data_dir.join("registry.json"), &self.registry)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn list(&self) -> Result<Vec<SkillSummary>> {
        let mut result = vec![];
        for entry in fs::read_dir(&self.config.library_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() || !path.join("SKILL.md").is_file() {
                continue;
            }
            let storage_name = entry.file_name().to_string_lossy().into_owned();
            let (_, description) = read_frontmatter(&path.join("SKILL.md")).unwrap_or_default();
            let mut record = self
                .registry
                .skills
                .get(&storage_name)
                .cloned()
                .unwrap_or_else(|| SkillRecord {
                    name: storage_name.clone(),
                    storage_name: storage_name.clone(),
                    description: String::new(),
                    origin: SkillOrigin::Unknown,
                    update_available: false,
                    group_id: None,
                    last_operated_at: 0,
                });
            if record.storage_name.is_empty() {
                record.storage_name = storage_name.clone();
            }
            record.description = description.unwrap_or_default();
            result.push(SkillSummary {
                installed_projects: self.install_count(&record.name, &path),
                record,
                manual_only: crate::effective_skills::is_manual_skill(&path),
                path,
            });
        }
        result.sort_by(|a, b| {
            b.record
                .origin
                .is_builtin()
                .cmp(&a.record.origin.is_builtin())
                .then_with(|| a.record.name.cmp(&b.record.name))
        });
        Ok(result)
    }

    fn install_count(&self, name: &str, source: &Path) -> usize {
        self.config
            .recent_projects
            .iter()
            .filter(|project| crate::project::is_installed_any_from_path(project, name, source))
            .count()
    }

    pub fn skill_path(&self, name: &str) -> Result<PathBuf> {
        Ok(self.resolve_skill(name)?.path)
    }

    /// Resolve a user-facing Skill selector. Display names must be unique;
    /// `id:<storage-name>` explicitly selects one of several same-named Skills.
    pub fn resolve_skill(&self, selector: &str) -> Result<SkillSummary> {
        if let Some(storage_name) = selector.strip_prefix("id:") {
            validate_name(storage_name)?;
            return self
                .list()?
                .into_iter()
                .find(|skill| skill.record.storage_name == storage_name)
                .with_context(|| format!("没有找到技能：{selector}"));
        }
        validate_name(selector)?;
        let skills = self.list()?;
        let matches = skills
            .into_iter()
            .filter(|skill| skill.record.name == selector)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [skill] => Ok(skill.clone()),
            [] => bail!("没有找到技能：{selector}"),
            _ => bail!("存在多个同名技能，请使用 id:<值> 明确选择"),
        }
    }

    pub fn skill_path_by_storage(&self, storage_name: &str) -> Result<PathBuf> {
        validate_name(storage_name)?;
        let path = self.config.library_dir.join(storage_name);
        if !path.join("SKILL.md").is_file() {
            bail!("没有找到技能：{storage_name}");
        }
        Ok(path)
    }

    pub fn files(&self, name: &str) -> Result<Vec<PathBuf>> {
        let root = self.skill_path(name)?;
        self.files_in(&root)
    }

    pub fn files_by_storage(&self, storage_name: &str) -> Result<Vec<PathBuf>> {
        let root = self.skill_path_by_storage(storage_name)?;
        self.files_in(&root)
    }

    fn files_in(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.path().strip_prefix(&root).ok().map(Path::to_path_buf))
            .collect::<Vec<_>>();
        files.sort();
        Ok(files)
    }

    pub fn read_file(&self, name: &str, relative: &Path) -> Result<String> {
        let root = self.skill_path(name)?.canonicalize()?;
        self.read_file_in(&root, relative)
    }

    pub fn read_file_by_storage(&self, storage_name: &str, relative: &Path) -> Result<String> {
        let root = self.skill_path_by_storage(storage_name)?.canonicalize()?;
        self.read_file_in(&root, relative)
    }

    fn read_file_in(&self, root: &Path, relative: &Path) -> Result<String> {
        let path = root.join(relative).canonicalize()?;
        if !path.starts_with(&root) {
            bail!("文件不在技能目录内");
        }
        fs::read_to_string(path).context("该文件不是可预览的文本文件")
    }

    pub fn import(&mut self, source: &Path, record: SkillRecord) -> Result<()> {
        if !source.join("SKILL.md").is_file() {
            bail!("所选目录中没有 SKILL.md");
        }
        validate_name(&record.name)?;
        let identity = record.identity_key();
        if self
            .registry
            .skills
            .values()
            .any(|existing| existing.identity_key() == identity)
        {
            bail!("这个来源中的技能已添加：{}", record.name);
        }
        let storage_name = self.storage_name_for(&record.name, &identity);
        let destination = self.config.library_dir.join(&storage_name);
        copy_tree(source, &destination)?;
        let mut record = record;
        record.storage_name = storage_name.clone();
        if record.last_operated_at == 0 {
            record.last_operated_at = operation_stamp();
        }
        if record.group_id.as_deref().is_some_and(|group_id| {
            !self
                .registry
                .groups
                .iter()
                .any(|group| group.id == group_id)
        }) {
            record.group_id = None;
        }
        self.registry.skills.insert(storage_name, record);
        self.save()
    }

    /// Adopt one source and its observed links as a single recoverable operation.
    /// Source files and native plugin registries are never written here.
    pub fn adopt(
        &mut self,
        candidate: &crate::adoption::AdoptionCandidate,
        references: &[crate::adoption::SkillReference],
    ) -> Result<String> {
        use crate::{adoption, directory_link};
        candidate.verify()?;
        if let Some(issue) = &candidate.issue {
            bail!("{issue}");
        }
        validate_name(&candidate.name)?;
        let mut seen = HashSet::new();
        let references = references
            .iter()
            .filter(|r| seen.insert(crate::project::installation_key(&r.path)))
            .cloned()
            .collect::<Vec<_>>();
        for reference in &references {
            adoption::validate_reference(reference)?;
        }
        let before = self.registry.clone();
        let identity = candidate.identity();
        let existing = self
            .registry
            .skills
            .values()
            .find(|r| r.identity_key() == identity)
            .cloned();
        let storage = existing
            .as_ref()
            .map(|r| r.storage_name.clone())
            .unwrap_or_else(|| self.storage_name_for(&candidate.name, &identity));
        let destination = self.config.library_dir.join(&storage);
        let already_points_here =
            destination.canonicalize().ok().as_ref() == Some(&candidate.source);
        let mut backup = None;
        let mut created = false;
        let mut changed = Vec::new();
        let result = (|| -> Result<()> {
            // Resolve all snapshots before changing anything. A link can point through
            // another reference or through the library entry that this operation switches.
            for reference in &references {
                if reference.kind != adoption::ReferenceKind::Link {
                    continue;
                }
                if crate::project::installation_key(&reference.path) == candidate.source {
                    continue;
                }
                let current =
                    directory_link::inspect(&reference.path)?.context("引用已变化，请重新扫描")?;
                if Some(&current.target) != reference.original_target.as_ref() {
                    bail!("引用已变化，请重新扫描：{}", reference.path.display());
                }
                if reference.original_target.as_ref() != Some(&candidate.source) {
                    adoption::replace_link(&reference.path, &candidate.source)?;
                    changed.push(reference.clone());
                }
                if reference.path.canonicalize()? != candidate.source {
                    bail!("引用验证失败：{}", reference.path.display());
                }
            }
            if !already_points_here {
                if destination.symlink_metadata().is_ok() {
                    // Keep an existing Kitter materialization recoverable, never overwrite it.
                    let parent = self.config.library_dir.join(".adoption-backups");
                    fs::create_dir_all(&parent)?;
                    let dir = tempfile::Builder::new()
                        .prefix("source-")
                        .tempdir_in(&parent)?
                        .keep();
                    let old = dir.join(&storage);
                    fs::rename(&destination, &old)?;
                    backup = Some(old);
                }
                directory_link::create(&candidate.source, &destination)?;
                created = true;
            }
            let mut record = candidate.record();
            record.storage_name = storage.clone();
            record.group_id = existing.as_ref().and_then(|r| r.group_id.clone());
            record.last_operated_at = operation_stamp();
            let source = record.origin.source();
            let source_record = self
                .registry
                .sources
                .entry(source.key())
                .or_insert_with(|| SkillSourceRecord {
                    source: source.clone(),
                    discovered_skills: Vec::new(),
                    added_skills: Vec::new(),
                });
            for list in [
                &mut source_record.discovered_skills,
                &mut source_record.added_skills,
            ] {
                if !list.contains(&record.name) {
                    list.push(record.name.clone());
                    list.sort();
                }
            }
            self.registry.skills.insert(storage.clone(), record);
            let mut owned = self
                .registry
                .adopted_sources
                .get(&storage)
                .map(|s| s.references.clone())
                .unwrap_or_default();
            for mut reference in references.clone() {
                if reference.kind != adoption::ReferenceKind::Link {
                    continue;
                }
                reference.source = candidate.source.clone();
                reference.original_target = Some(candidate.source.clone());
                owned.retain(|r| {
                    crate::project::installation_key(&r.path)
                        != crate::project::installation_key(&reference.path)
                });
                owned.push(reference);
            }
            self.registry.adopted_sources.insert(
                storage.clone(),
                AdoptedSource {
                    source: candidate.source.clone(),
                    references: owned,
                    previous_library: backup.clone().or_else(|| {
                        before
                            .adopted_sources
                            .get(&storage)
                            .and_then(|s| s.previous_library.clone())
                    }),
                },
            );
            self.save()?;
            Ok(())
        })();
        if let Err(error) = result {
            let mut failures = adoption::rollback_links(&changed);
            if created {
                if let Ok(Some(link)) = directory_link::inspect(&destination) {
                    if let Err(e) = directory_link::remove(&destination, link) {
                        failures.push(e.to_string());
                    }
                }
            }
            if let Some(path) = &backup {
                if let Err(e) = fs::rename(path, &destination) {
                    failures.push(e.to_string());
                }
            }
            self.registry = before;
            if failures.is_empty() {
                return Err(error);
            }
            bail!("{error}；回滚未完成：{}", failures.join("；"));
        }
        Ok(storage)
    }

    pub fn is_linked_source(&self, storage_name: &str) -> bool {
        crate::directory_link::inspect(&self.config.library_dir.join(storage_name))
            .ok()
            .flatten()
            .is_some()
    }

    pub fn replace(&mut self, source: &Path, record: SkillRecord) -> Result<()> {
        let storage_name = if record.storage_name.is_empty() {
            record.name.clone()
        } else {
            record.storage_name.clone()
        };
        self.replace_by_storage(source, storage_name, record)
    }

    pub fn replace_by_storage(
        &mut self,
        source: &Path,
        storage_name: String,
        mut record: SkillRecord,
    ) -> Result<()> {
        if self.is_linked_source(&storage_name) {
            bail!("此技能链接到原始目录，请在来源中更新");
        }
        let affected_projects = self
            .config
            .recent_projects
            .iter()
            .filter(|project| {
                crate::project::is_installed_any_from_path(
                    project,
                    &record.name,
                    &self.config.library_dir.join(&storage_name),
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let destination = self.skill_path_by_storage(&storage_name)?;
        let backup = destination.with_extension("kitter-backup");
        if backup.exists() {
            fs::remove_dir_all(&backup)?;
        }
        fs::rename(&destination, &backup)?;
        match copy_tree(source, &destination) {
            Ok(()) => {
                fs::remove_dir_all(backup)?;
                record.storage_name = storage_name.clone();
                record.last_operated_at = operation_stamp();
                self.registry.skills.insert(storage_name, record);
                for project in affected_projects {
                    self.config.touch_project(&project);
                }
                self.save()
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&destination);
                fs::rename(backup, destination)?;
                Err(error)
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        let record = self.record(name)?;
        self.remove_by_storage(&record.storage_name)
    }

    pub fn remove_by_storage(&mut self, storage_name: &str) -> Result<()> {
        let record = self.record_by_storage(storage_name)?;
        if record.origin.is_builtin() {
            bail!("Kitter 内置 Skill 不能删除");
        }
        let source_key = record.origin.source().key();
        let path = self.skill_path_by_storage(storage_name)?;
        let affected_projects = self
            .config
            .recent_projects
            .iter()
            .filter(|project| {
                crate::project::is_installed_any_from_path(project, &record.name, &path)
            })
            .cloned()
            .collect::<Vec<_>>();
        for project in &self.config.recent_projects {
            crate::project::uninstall_all_from_path(project, &record.name, &path)?;
        }
        if let Some(home) = dirs::home_dir() {
            crate::project::uninstall_all_from_path(&home, &record.name, &path)?;
        }
        if let Some(adopted) = self.registry.adopted_sources.get(storage_name) {
            for reference in &adopted.references {
                if reference.kind != crate::adoption::ReferenceKind::Link {
                    continue;
                }
                if let Some(link) = crate::directory_link::inspect(&reference.path)? {
                    // Never remove a reference that another tool/user has repointed.
                    if reference.path.canonicalize().ok().as_ref() == Some(&adopted.source) {
                        crate::directory_link::remove(&reference.path, link)?;
                    }
                }
            }
        }
        if let Some(link) = crate::directory_link::inspect(&path)? {
            crate::directory_link::remove(&path, link)?;
        } else {
            fs::remove_dir_all(path)?;
        }
        self.registry.skills.remove(storage_name);
        self.registry.adopted_sources.remove(storage_name);
        if let Some(source) = self.registry.sources.get_mut(&source_key) {
            source.added_skills.retain(|skill| skill != &record.name);
        }
        for project in affected_projects {
            self.config.touch_project(&project);
        }
        self.save()
    }

    pub fn record_source(
        &mut self,
        source: SkillSource,
        mut discovered_skills: Vec<String>,
        added_skills: impl IntoIterator<Item = String>,
    ) -> Result<()> {
        discovered_skills.sort();
        discovered_skills.dedup();
        let key = source.key();
        let record = self
            .registry
            .sources
            .entry(key)
            .or_insert_with(|| SkillSourceRecord {
                source: source.clone(),
                discovered_skills: Vec::new(),
                added_skills: Vec::new(),
            });
        record.source = source;
        record.discovered_skills = discovered_skills;
        record.added_skills.extend(added_skills);
        record.added_skills.sort();
        record.added_skills.dedup();
        self.save()
    }

    pub fn source_records(&self) -> Vec<SkillSourceRecord> {
        let mut sources = self.registry.sources.values().cloned().collect::<Vec<_>>();
        sources.sort_by_key(|source| source.source.label());
        sources
    }

    pub fn groups(&self) -> Vec<SkillGroup> {
        self.registry.groups.clone()
    }

    pub fn ensure_group(&mut self, name: &str) -> Result<String> {
        let name = normalize_group_name(name)?;
        if let Some(group) = self
            .registry
            .groups
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(&name))
        {
            return Ok(group.id.clone());
        }
        let id = self.new_group_id();
        self.registry.groups.push(SkillGroup {
            id: id.clone(),
            name,
            created_at: operation_stamp(),
        });
        self.save()?;
        Ok(id)
    }

    pub fn create_group(&mut self, name: &str) -> Result<SkillGroup> {
        let name = normalize_group_name(name)?;
        if self
            .registry
            .groups
            .iter()
            .any(|group| group.name.eq_ignore_ascii_case(&name))
        {
            bail!("已经存在同名分组");
        }
        let group = SkillGroup {
            id: self.new_group_id(),
            name,
            created_at: operation_stamp(),
        };
        self.registry.groups.push(group.clone());
        self.save()?;
        Ok(group)
    }

    pub fn rename_group(&mut self, id: &str, name: &str) -> Result<()> {
        let name = normalize_group_name(name)?;
        if self
            .registry
            .groups
            .iter()
            .any(|group| group.id != id && group.name.eq_ignore_ascii_case(&name))
        {
            bail!("已经存在同名分组");
        }
        let group = self
            .registry
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .context("分组不存在")?;
        group.name = name;
        self.save()
    }

    pub fn assign_group(&mut self, skill: &str, group_id: Option<&str>) -> Result<()> {
        if self.registry.skills.contains_key(skill) {
            return self.assign_group_by_storage(skill, group_id);
        }
        let record = self.record(skill)?;
        self.assign_group_by_storage(&record.storage_name, group_id)
    }

    pub fn delete_group(&mut self, id: &str, delete_skills: bool) -> Result<Vec<String>> {
        let storage_names = self
            .registry
            .skills
            .values()
            .filter(|record| record.group_id.as_deref() == Some(id))
            .map(|record| {
                if record.storage_name.is_empty() {
                    record.name.clone()
                } else {
                    record.storage_name.clone()
                }
            })
            .collect::<Vec<_>>();
        if delete_skills {
            for storage_name in &storage_names {
                self.remove_by_storage(storage_name)?;
            }
        } else {
            for record in self.registry.skills.values_mut() {
                if record.group_id.as_deref() == Some(id) {
                    record.group_id = None;
                    record.last_operated_at = operation_stamp();
                }
            }
        }
        self.registry.groups.retain(|group| group.id != id);
        self.save()?;
        Ok(storage_names)
    }

    fn new_group_id(&self) -> String {
        let stamp = operation_stamp();
        let mut id = format!("group-{stamp}");
        let mut suffix = 1;
        while self.registry.groups.iter().any(|group| group.id == id) {
            id = format!("group-{stamp}-{suffix}");
            suffix += 1;
        }
        id
    }

    pub fn record(&self, name: &str) -> Result<SkillRecord> {
        Ok(self.resolve_skill(name)?.record)
    }

    pub fn record_by_storage(&self, storage_name: &str) -> Result<SkillRecord> {
        self.list()?
            .into_iter()
            .find(|skill| skill.record.storage_name == storage_name)
            .map(|skill| skill.record)
            .context("技能不存在")
    }

    pub fn set_update_available(&mut self, name: &str, available: bool) -> Result<()> {
        let record = self.record(name)?;
        self.set_update_available_by_storage(&record.storage_name, available)
    }

    pub fn set_update_available_by_storage(
        &mut self,
        storage_name: &str,
        available: bool,
    ) -> Result<()> {
        let record = self
            .registry
            .skills
            .get_mut(storage_name)
            .context("技能不存在")?;
        record.update_available = available;
        self.save()
    }

    pub fn assign_group_by_storage(
        &mut self,
        storage_name: &str,
        group_id: Option<&str>,
    ) -> Result<()> {
        if self
            .registry
            .skills
            .get(storage_name)
            .is_some_and(|record| record.origin.is_builtin())
        {
            bail!("Kitter 内置 Skill 固定显示在列表顶部");
        }
        if let Some(group_id) = group_id
            && !self
                .registry
                .groups
                .iter()
                .any(|group| group.id == group_id)
        {
            bail!("分组不存在");
        }
        let record = self
            .registry
            .skills
            .get_mut(storage_name)
            .context("技能不存在")?;
        record.group_id = group_id.map(str::to_string);
        record.last_operated_at = operation_stamp();
        self.save()
    }

    fn storage_name_for(&self, name: &str, identity: &str) -> String {
        let direct = self.config.library_dir.join(name);
        if !direct.exists() && !self.registry.skills.contains_key(name) {
            return name.to_string();
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        identity.hash(&mut hasher);
        let suffix = format!("{:08x}", hasher.finish() as u32);
        let base = format!("{name}--{suffix}");
        let mut candidate = base.clone();
        let mut index = 2;
        while self.config.library_dir.join(&candidate).exists()
            || self.registry.skills.contains_key(&candidate)
        {
            candidate = format!("{base}-{index}");
            index += 1;
        }
        candidate
    }
}

fn ensure_builtin_skill(library_dir: &Path) -> Result<()> {
    let root = library_dir.join(KITTER_SKILL_STORAGE);
    let agents = root.join("agents");
    let references = root.join("references");
    fs::create_dir_all(&agents)?;
    fs::create_dir_all(&references)?;
    write_builtin_file(&root.join("SKILL.md"), KITTER_SKILL_MD)?;
    write_builtin_file(&agents.join("openai.yaml"), KITTER_OPENAI_YAML)?;
    write_builtin_file(&references.join("install-cli.md"), KITTER_INSTALL_CLI_MD)?;
    if let Some(source) = bundled_cli_source() {
        install_builtin_cli_asset(&source, &root)?;
    }
    Ok(())
}

fn write_builtin_file(path: &Path, content: &str) -> Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        fs::write(path, content)?;
    }
    Ok(())
}

fn bundled_cli_source() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KITTER_BUNDLED_CLI").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }

    let executable = std::env::current_exe().ok()?;
    let binary_name = if cfg!(target_os = "windows") {
        "kitter.exe"
    } else {
        "kitter"
    };
    if executable
        .file_name()
        .is_some_and(|name| name == binary_name)
    {
        return Some(executable);
    }

    let executable_dir = executable.parent()?;
    let macos_packaged = executable_dir.parent().map(|contents| {
        contents
            .join("Resources/kitter-skill/bin")
            .join(binary_name)
    });
    if macos_packaged.as_ref().is_some_and(|path| path.is_file()) {
        return macos_packaged;
    }

    let packaged = executable_dir
        .join("resources/kitter-skill/bin")
        .join(binary_name);
    if packaged.is_file() {
        return Some(packaged);
    }

    let sibling = executable_dir.join(binary_name);
    sibling.is_file().then_some(sibling)
}

fn install_builtin_cli_asset(source: &Path, skill_root: &Path) -> Result<()> {
    let binary_name = if cfg!(target_os = "windows") {
        "kitter.exe"
    } else {
        "kitter"
    };
    let destination = skill_root.join("bin").join(binary_name);
    if fs::canonicalize(source).ok() == fs::canonicalize(&destination).ok()
        || fs::read(source).ok() == fs::read(&destination).ok()
    {
        return Ok(());
    }
    fs::create_dir_all(destination.parent().expect("CLI destination has a parent"))?;
    fs::copy(source, &destination).with_context(|| {
        format!(
            "安装 Kitter CLI 失败：{} -> {}",
            source.display(),
            destination.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn operation_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn normalize_group_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("请输入分组名称");
    }
    if name.contains('/') || name.contains('\\') {
        bail!("分组名称不能包含路径分隔符");
    }
    Ok(name.to_string())
}

fn unique_group_name(name: &str, used_names: &mut HashSet<String>) -> String {
    let base = name.to_string();
    let mut candidate = base.clone();
    let mut suffix = 2;
    while used_names
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&candidate))
    {
        candidate = format!("{base} {suffix}");
        suffix += 1;
    }
    used_names.insert(candidate.clone());
    candidate
}

fn migrated_group_id(source_key: &str, groups: &[SkillGroup]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source_key.hash(&mut hasher);
    let base = format!("source-{:08x}", hasher.finish() as u32);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while groups.iter().any(|group| group.id == candidate) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

pub fn read_frontmatter(path: &Path) -> Result<(Option<String>, Option<String>)> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    if lines.next().is_some_and(|line| line.trim() == "---") {
        let frontmatter = lines
            .take_while(|line| line.trim() != "---")
            .collect::<Vec<_>>();
        if let Ok(metadata) = serde_yaml::from_str::<Frontmatter>(&frontmatter.join("\n")) {
            return Ok((metadata.name, metadata.description.map(trim_description)));
        }
        let mut name = None;
        let mut description = None;
        for line in frontmatter {
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(unquote(value));
            }
            if let Some(value) = line.strip_prefix("description:") {
                description = Some(unquote(value));
            }
        }
        return Ok((name, description));
    }
    Ok((None, None))
}

#[derive(Default, Deserialize)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

fn trim_description(description: String) -> String {
    description.trim_end().to_string()
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).to_string()
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        bail!("技能名称无效");
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        if relative
            .components()
            .next()
            .is_some_and(|part| part.as_os_str() == ".git")
        {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_cli_skill_is_manual_pinned_and_protected() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("data");
        let library_dir = data_dir.join("skills");
        fs::create_dir_all(&library_dir).unwrap();
        fixture_skill(&library_dir.join("another-skill"), "another-skill");
        ensure_builtin_skill(&library_dir).unwrap();

        let builtin_path = library_dir.join(KITTER_SKILL_STORAGE);
        assert!(crate::effective_skills::is_manual_skill(&builtin_path));
        let skill_markdown = fs::read_to_string(builtin_path.join("SKILL.md")).unwrap();
        assert!(skill_markdown.contains("kitter install"));
        assert!(skill_markdown.contains("bin/kitter"));
        assert!(skill_markdown.contains("disable-model-invocation: true"));
        assert!(skill_markdown.contains("autoinvoke: false"));
        assert!(
            fs::read_to_string(builtin_path.join("references/install-cli.md"))
                .unwrap()
                .contains("gh release download")
        );
        assert!(
            fs::read_to_string(builtin_path.join("agents/openai.yaml"))
                .unwrap()
                .contains("allow_implicit_invocation: false")
        );

        let fixture_cli = temp.path().join(if cfg!(target_os = "windows") {
            "fixture-kitter.exe"
        } else {
            "fixture-kitter"
        });
        fs::write(&fixture_cli, b"first build").unwrap();
        install_builtin_cli_asset(&fixture_cli, &builtin_path).unwrap();
        let installed_cli = builtin_path
            .join("bin")
            .join(if cfg!(target_os = "windows") {
                "kitter.exe"
            } else {
                "kitter"
            });
        assert_eq!(fs::read(&installed_cli).unwrap(), b"first build");
        fs::write(&fixture_cli, b"updated build").unwrap();
        install_builtin_cli_asset(&fixture_cli, &builtin_path).unwrap();
        assert_eq!(fs::read(&installed_cli).unwrap(), b"updated build");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&installed_cli).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }

        let mut registry = Registry::default();
        registry.skills.insert(
            KITTER_SKILL_STORAGE.to_string(),
            SkillRecord {
                name: KITTER_SKILL_NAME.to_string(),
                storage_name: KITTER_SKILL_STORAGE.to_string(),
                description: String::new(),
                origin: SkillOrigin::Builtin,
                update_available: false,
                group_id: None,
                last_operated_at: u64::MAX,
            },
        );
        let mut library = SkillLibrary {
            config: AppConfig {
                library_dir,
                ..AppConfig::default()
            },
            registry,
            data_dir,
        };
        let skills = library.list().unwrap();
        assert_eq!(skills[0].record.name, KITTER_SKILL_NAME);
        assert!(skills[0].record.origin.is_builtin());
        assert!(library.remove_by_storage(KITTER_SKILL_STORAGE).is_err());
        assert!(
            library
                .assign_group_by_storage(KITTER_SKILL_STORAGE, None)
                .is_err()
        );
    }

    #[cfg(unix)]
    fn adoption_fixture() -> (tempfile::TempDir, SkillLibrary, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let home = root.join("home");
        let library_dir = root.join("data/skills");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&library_dir).unwrap();
        let library = SkillLibrary {
            config: AppConfig {
                library_dir,
                ..AppConfig::default()
            },
            registry: Registry::default(),
            data_dir: root.join("data"),
        };
        (temp, library, home)
    }

    fn fixture_skill(path: &Path, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n"),
        )
        .unwrap();
        fs::write(path.join("keep.txt"), "original source").unwrap();
    }

    #[cfg(unix)]
    fn discover(library: &SkillLibrary, home: &Path) -> crate::adoption::AdoptionScan {
        crate::adoption::scan(
            home,
            &[],
            &library.config.library_dir,
            &library.list().unwrap(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn adoption_links_library_and_all_references_without_copying_or_deleting_source() {
        use std::os::unix::fs::symlink;
        let (_temp, mut library, home) = adoption_fixture();
        let source = home.join("source");
        fixture_skill(&source, "demo");
        fs::create_dir_all(home.join(".claude/skills")).unwrap();
        let link = home.join(".claude/skills/demo");
        symlink("../../source", &link).unwrap();
        symlink(&source, home.join("custom-alias")).unwrap();
        let scan = discover(&library, &home);
        let candidate = &scan.candidates[0];
        let storage = library
            .adopt(candidate, &scan.references_for(candidate))
            .unwrap();
        assert!(library.is_linked_source(&storage));
        assert!(crate::source::update_by_storage(&mut library, &storage).is_err());
        assert_eq!(fs::read_link(&link).unwrap(), source);
        assert_eq!(
            crate::project::list(&home, &library.config.library_dir).unwrap()[0].installations[0]
                .managed,
            true
        );
        let scan = discover(&library, &home);
        library
            .adopt(
                &scan.candidates[0],
                &scan.references_for(&scan.candidates[0]),
            )
            .unwrap();
        assert_eq!(library.list().unwrap().len(), 1);
        library.remove_by_storage(&storage).unwrap();
        assert_eq!(
            fs::read_to_string(source.join("keep.txt")).unwrap(),
            "original source"
        );
        assert!(link.symlink_metadata().is_err());
        assert_eq!(home.join("custom-alias").canonicalize().unwrap(), source); // Outside direct install roots.
    }

    #[cfg(unix)]
    #[test]
    fn adoption_rolls_back_links_and_registration_when_save_fails() {
        use std::os::unix::fs::symlink;
        let (_temp, mut library, home) = adoption_fixture();
        fixture_skill(&home.join("source"), "demo");
        fs::create_dir_all(home.join(".claude/skills")).unwrap();
        symlink("../../source", home.join(".claude/skills/alias")).unwrap();
        let scan = discover(&library, &home);
        library.data_dir = home.join("source/SKILL.md");
        assert!(
            library
                .adopt(
                    &scan.candidates[0],
                    &scan.references_for(&scan.candidates[0])
                )
                .is_err()
        );
        assert_eq!(
            home.join(".claude/skills/alias").canonicalize().unwrap(),
            home.join("source")
        );
        assert!(library.list().unwrap().is_empty());
        assert!(library.registry.skills.is_empty());
        assert!(home.join("source/keep.txt").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn adopted_same_names_keep_distinct_source_identities_and_storage() {
        let (_temp, mut library, home) = adoption_fixture();
        fixture_skill(&home.join(".agents/skills/a"), "same");
        fixture_skill(&home.join(".agents/skills/b"), "same");
        let scan = discover(&library, &home);
        for candidate in &scan.candidates {
            library.adopt(candidate, &[]).unwrap();
        }
        let skills = library.list().unwrap();
        assert_eq!(skills.len(), 2);
        assert_ne!(
            skills[0].record.identity_key(),
            skills[1].record.identity_key()
        );
        assert_ne!(skills[0].record.storage_name, skills[1].record.storage_name);
        assert!(library.resolve_skill("same").is_err());
        for skill in skills {
            let resolved = library
                .resolve_skill(&format!("id:{}", skill.record.storage_name))
                .unwrap();
            assert_eq!(resolved.path, skill.path);
        }
    }

    #[cfg(unix)]
    #[test]
    fn switching_an_identity_updates_chained_and_library_references() {
        use std::os::unix::fs::symlink;
        for fail_save in [false, true] {
            let (_temp, mut library, home) = adoption_fixture();
            for project in ["a", "b"] {
                let root = home.join(project);
                fixture_skill(&root.join(".agents/skills/alias"), "demo");
                fs::write(
                    root.join("skills-lock.json"),
                    r#"{"skills":{"alias":{"source":"owner/repo","sourceType":"github"}}}"#,
                )
                .unwrap();
            }
            let a = home.join("a/.agents/skills/alias");
            let b = home.join("b/.agents/skills/alias");
            let scan = discover(&library, &home);
            let first = scan.candidates.iter().find(|c| c.source == a).unwrap();
            let storage = library.adopt(first, &[]).unwrap();
            let library_path = library.config.library_dir.join(&storage);
            fs::create_dir_all(home.join(".codex/skills")).unwrap();
            symlink(&library_path, home.join(".codex/skills/library-ref")).unwrap();
            symlink(&a, home.join(".codex/skills/first-ref")).unwrap();
            symlink(
                home.join(".codex/skills/first-ref"),
                home.join(".codex/skills/chained-ref"),
            )
            .unwrap();
            let scan = discover(&library, &home);
            let second = scan.candidates.iter().find(|c| c.source == b).unwrap();
            assert_eq!(scan.references_for(second).len(), 5);
            if fail_save {
                library.data_dir = home.join("a/.agents/skills/alias/SKILL.md");
            }
            let result = library.adopt(second, &scan.references_for(second));
            assert_eq!(result.is_err(), fail_save);
            let expected = if fail_save { &a } else { &b };
            for reference in ["library-ref", "first-ref", "chained-ref"] {
                assert_eq!(
                    &home
                        .join(".codex/skills")
                        .join(reference)
                        .canonicalize()
                        .unwrap(),
                    expected
                );
            }
            assert_eq!(&library_path.canonicalize().unwrap(), expected);
            assert!(a.join("keep.txt").is_file());
            assert!(b.join("keep.txt").is_file());
            assert_eq!(library.list().unwrap().len(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn removal_does_not_delete_a_repointed_reference_or_a_direct_source_directory() {
        use std::os::unix::fs::symlink;
        let (_temp, mut library, home) = adoption_fixture();
        let source = home.join(".agents/skills/demo");
        fixture_skill(&source, "demo");
        fixture_skill(&home.join("other"), "other");
        fs::create_dir_all(home.join(".claude/skills")).unwrap();
        symlink(&source, home.join(".claude/skills/alias")).unwrap();
        let scan = discover(&library, &home);
        let candidate = scan.candidates.iter().find(|c| c.name == "demo").unwrap();
        let storage = library
            .adopt(candidate, &scan.references_for(candidate))
            .unwrap();
        assert!(
            !crate::project::list(&home, &library.config.library_dir).unwrap()[0].installations[0]
                .managed
        );
        crate::adoption::replace_link(&home.join(".claude/skills/alias"), &home.join("other"))
            .unwrap();
        library.remove_by_storage(&storage).unwrap();
        assert!(source.join("keep.txt").is_file());
        assert_eq!(
            home.join(".claude/skills/alias").canonicalize().unwrap(),
            home.join("other")
        );
    }

    #[cfg(unix)]
    #[test]
    fn direct_install_scan_does_not_adopt_or_modify_plugins() {
        let (_temp, library, home) = adoption_fixture();
        let root = home.join(".claude/plugins/cache/market/demo/v1");
        fixture_skill(&root.join("skills/demo"), "demo");
        let registry_path = home.join(".claude/plugins/installed_plugins.json");
        let bytes = serde_json::to_vec(
            &serde_json::json!({"plugins":{"demo@market":[{"scope":"user","installPath":root}]}}),
        )
        .unwrap();
        fs::write(&registry_path, &bytes).unwrap();
        let scan = discover(&library, &home);
        assert!(scan.candidates.is_empty());
        assert_eq!(fs::read(registry_path).unwrap(), bytes);
        assert!(root.join("skills/demo/keep.txt").is_file());
    }

    #[test]
    fn reads_literal_block_descriptions() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("SKILL.md");
        fs::write(
            &path,
            "---\nname: multiline\ndescription: |\n  First line\n  Second line\n---\n",
        )
        .unwrap();

        assert_eq!(
            read_frontmatter(&path).unwrap(),
            (
                Some("multiline".into()),
                Some("First line\nSecond line".into())
            )
        );
    }

    #[test]
    fn list_reads_the_current_description_without_persisting_it() {
        let temp = tempfile::tempdir().unwrap();
        let library_dir = temp.path().join("skills");
        let skill_dir = library_dir.join("demo");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, "---\nname: demo\ndescription: first\n---\n").unwrap();

        let library = SkillLibrary {
            config: AppConfig {
                library_dir,
                ..AppConfig::default()
            },
            registry: Registry::default(),
            data_dir: temp.path().join("data"),
        };

        assert_eq!(library.list().unwrap()[0].record.description, "first");
        fs::write(&skill_file, "---\nname: demo\ndescription: second\n---\n").unwrap();
        assert_eq!(library.list().unwrap()[0].record.description, "second");

        let json = serde_json::to_string(&library.list().unwrap()[0].record).unwrap();
        assert!(!json.contains("description"));
    }
}
