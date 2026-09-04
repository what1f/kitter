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

pub struct ScannedSkill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

enum ScanOrigin {
    Npx {
        repository: String,
        workspace: PathBuf,
    },
    Claude {
        plugin: String,
    },
    Local {
        root: PathBuf,
        label: String,
    },
}

pub struct SkillScan {
    origin: ScanOrigin,
    skills: Vec<ScannedSkill>,
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
        if selected.is_empty() {
            bail!("请至少选择一个技能");
        }
        let SkillScan {
            origin,
            skills,
            _temp: _,
        } = self;
        let discovered_skills = skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>();
        let mut added_skills = Vec::new();
        let mut skipped = 0;
        let group_id = group_name
            .filter(|name| !name.trim().is_empty())
            .map(|name| library.ensure_group(name))
            .transpose()?;
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
                }
                .identity_key(&skill.name),
            };
            if library.contains_identity(&identity) {
                skipped += 1;
                continue;
            }
            let (source_path, skill_origin) = match &origin {
                ScanOrigin::Npx {
                    repository,
                    workspace,
                } => {
                    // Keep a reusable upstream workspace, but only ask the
                    // upstream CLI for Skills that are not already present.
                    // Re-running `--skill *` can abort on its own duplicates
                    // before Kitter gets a chance to skip them.
                    ensure_npx_skill(workspace, repository, &skill.name)?;
                    (
                        npx_skill_path(workspace, &skill.name),
                        SkillOrigin::Npx {
                            repository: repository.clone(),
                            skill: skill.name.clone(),
                            source_hash: npx_lock_hash(workspace, &skill.name)?,
                        },
                    )
                }
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
                    },
                ),
            };
            let name = skill.name.clone();
            library.import(
                &source_path,
                SkillRecord {
                    name: skill.name,
                    storage_name: String::new(),
                    description: skill.description,
                    origin: skill_origin,
                    update_available: false,
                    group_id: group_id.clone(),
                    last_operated_at: 0,
                },
            )?;
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
        bail!("请选择一个文件夹");
    }

    let mut skill_dirs = BTreeSet::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        })
    {
        let entry = entry.with_context(|| format!("无法读取文件夹：{}", root.display()))?;
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
            .context("无法确定技能名称")?;
        if !names.insert(name.clone()) {
            bail!("发现多个名为 {name} 的技能，请调整名称后重试");
        }
        skills.push(ScannedSkill {
            name,
            description: description.unwrap_or_default(),
            path,
        });
    }

    if skills.is_empty() {
        bail!("这个文件夹中没有找到可用的技能");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Local {
            root: root.to_path_buf(),
            label: root.display().to_string(),
        },
        skills,
        _temp: None,
    })
}

pub fn scan_npx(input: &str) -> Result<SkillScan> {
    let repository = normalize_npx_source(input)?;
    let workspace = npx_workspace(&repository);
    let temp = TempDir::new()?;
    npx_add(temp.path(), &repository, "*")?;
    let root = temp.path().join(".agents/skills");
    let mut skills = Vec::new();
    for entry in
        fs::read_dir(&root).with_context(|| format!("没有从来源中找到技能：{}", repository))?
    {
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
            .context("无法确定技能名称")?;
        skills.push(ScannedSkill {
            name: skill,
            description: description.unwrap_or_default(),
            path,
        });
    }
    if skills.is_empty() {
        bail!("这个地址中没有找到可用的技能");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Npx {
            repository,
            workspace,
        },
        skills,
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
            .context("无法确定技能名称")?;
        skills.push(ScannedSkill {
            name: skill,
            description: description.unwrap_or_default(),
            path,
        });
    }
    if skills.is_empty() {
        bail!("这个插件中没有找到可用的技能");
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SkillScan {
        origin: ScanOrigin::Claude { plugin },
        skills,
        _temp: None,
    })
}

fn normalize_npx_source(input: &str) -> Result<String> {
    let value = input.trim();
    if value.is_empty() {
        bail!("请输入 skills.sh 或 GitHub 地址");
    }
    if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("git@") {
        return Ok(normalize_repository_url(value));
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
        .map(|value| normalize_repository_url(&value))
        .context("没有识别到 skills.sh 或 GitHub 地址")
}

fn normalize_repository_url(value: &str) -> String {
    if let Some(path) = value.strip_prefix("git@github.com:") {
        return format!("https://github.com/{}", path.trim_end_matches(".git"));
    }
    value.to_string()
}

fn normalize_claude_plugin(input: &str) -> Result<String> {
    let value = input.trim();
    if value.is_empty() {
        bail!("请输入 Claude 插件名称");
    }
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 1 {
        return Ok(parts[0].to_string());
    }
    parts
        .windows(2)
        .find_map(|pair| (pair[0] == "install").then(|| pair[1].to_string()))
        .context("没有识别到 Claude 插件名称")
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
        bail!("此技能链接到原始目录，请在来源中更新");
    }
    let temp = TempDir::new()?;
    let mut updated_origin = None;
    let source = match record.origin.clone() {
        SkillOrigin::Builtin => bail!("Kitter 内置 Skill 会随 Kitter 自动更新"),
        SkillOrigin::Npx {
            repository, skill, ..
        } => {
            let workspace = npx_workspace(&repository);
            ensure_npx_skill(&workspace, &repository, &skill)?;
            npx_update(&workspace, Some(&skill))?;
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
        SkillOrigin::Local { path, .. } => path.clone(),
        SkillOrigin::Unknown => bail!("这个技能没有可用的更新来源"),
    };
    if let Some(origin) = updated_origin {
        record.origin = origin;
    }
    record.update_available = false;
    library.replace_by_storage(&source, storage_name, record)
}

pub fn check_updates(library: &mut SkillLibrary) -> Result<usize> {
    let records = library
        .list()?
        .into_iter()
        .filter(|skill| {
            !skill.record.origin.is_builtin()
                && !library.is_linked_source(&skill.record.storage_name)
        })
        .map(|skill| skill.record)
        .collect::<Vec<_>>();

    // The upstream CLI owns Npx version detection. It updates the persistent
    // source workspace, then we compare its lock hash with the hash recorded
    // when the Kitter copy was last installed. This avoids reimplementing
    // repository/tree comparison here while preserving the UI's pending-update
    // state until the user chooses to install the update into the library.
    let mut npx_sources = BTreeMap::<String, Vec<String>>::new();
    for record in &records {
        if let SkillOrigin::Npx {
            repository, skill, ..
        } = &record.origin
        {
            npx_sources
                .entry(repository.clone())
                .or_default()
                .push(skill.clone());
        }
    }

    let mut npx_hashes = HashMap::<String, HashMap<String, String>>::new();
    let mut failures = Vec::new();
    for (repository, skills) in npx_sources {
        let result = (|| -> Result<HashMap<String, String>> {
            let workspace = npx_workspace(&repository);
            for skill in skills {
                ensure_npx_skill(&workspace, &repository, &skill)?;
            }
            // `skills update` performs the upstream check and refreshes only
            // the source workspace. The Kitter library remains unchanged
            // until the user presses the per-Skill update action.
            npx_update(&workspace, None)?;
            npx_lock_hashes(&workspace)
        })();
        match result {
            Ok(hashes) => {
                npx_hashes.insert(repository, hashes);
            }
            Err(error) => failures.push(format!("Npx 来源检查失败：{error:#}")),
        }
    }

    let mut count = 0;
    for record in records {
        let result = match &record.origin {
            SkillOrigin::Npx {
                repository,
                skill,
                source_hash,
            } => Ok(npx_hashes
                .get(repository)
                .and_then(|hashes| hashes.get(skill))
                .is_some_and(|current| source_hash.as_deref() != Some(current.as_str()))),
            _ => check_one(library, &record),
        };
        match result {
            Ok(available) => {
                library.set_update_available_by_storage(&record.storage_name, available)?;
                count += usize::from(available);
            }
            Err(error) => failures.push(format!("{}: {error:#}", record.name)),
        }
    }
    if !failures.is_empty() {
        bail!("部分技能检查失败：{}", failures.join("；"));
    }
    Ok(count)
}

fn check_one(library: &SkillLibrary, record: &SkillRecord) -> Result<bool> {
    let temp = TempDir::new()?;
    let source = match &record.origin {
        SkillOrigin::Builtin => return Ok(false),
        SkillOrigin::Git { repository, subdir } => {
            run(tool_command("git")
                .args(["clone", "--depth", "1", repository])
                .arg(temp.path().join("repo")))?;
            find_skill_dir(
                &subdir
                    .as_ref()
                    .map(|path| temp.path().join("repo").join(path))
                    .unwrap_or_else(|| temp.path().join("repo")),
            )?
        }
        SkillOrigin::ClaudeMarketplace { plugin, skill } => find_claude_skill(plugin, skill)?,
        SkillOrigin::Local { path, .. } => path.clone(),
        SkillOrigin::Unknown => return Ok(false),
        SkillOrigin::Npx { .. } => return Ok(false),
    };
    Ok(!same_tree(
        &source,
        &library.skill_path_by_storage(&record.storage_name)?,
    )?)
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
}

impl NpxLockEntry {
    fn hash(self) -> Option<String> {
        self.skill_folder_hash.or(self.computed_hash)
    }
}

fn npx_workspace(repository: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    repository.hash(&mut hasher);
    crate::config::app_data_dir()
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

fn npx_update(workspace: &Path, skill: Option<&str>) -> Result<()> {
    let mut command = tool_command("npx");
    command
        .current_dir(workspace)
        .args(["-y", "skills", "update", "--project", "--yes"]);
    if let Some(skill) = skill {
        command.arg(skill);
    }
    run(&mut command)
}

fn ensure_npx_skill(workspace: &Path, repository: &str, skill: &str) -> Result<()> {
    let has_lock = workspace.join("skills-lock.json").is_file()
        || workspace.join(".agents").join(".skill-lock.json").is_file();
    if !has_lock || !npx_skill_path(workspace, skill).join("SKILL.md").is_file() {
        npx_add(workspace, repository, skill)?;
    }
    Ok(())
}

fn npx_lock_hash(workspace: &Path, skill: &str) -> Result<Option<String>> {
    Ok(npx_lock_hashes(workspace)?.remove(skill))
}

fn npx_lock_hashes(workspace: &Path) -> Result<HashMap<String, String>> {
    let paths = [
        workspace.join(".agents").join(".skill-lock.json"),
        workspace.join("skills-lock.json"),
    ];
    let mut hashes = HashMap::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)
            .with_context(|| format!("读取 Npx lock 文件失败：{}", path.display()))?;
        let lock: NpxLockFile = serde_json::from_slice(&bytes)
            .with_context(|| format!("Npx lock 文件格式无效：{}", path.display()))?;
        for (name, entry) in lock.skills {
            if let Some(hash) = entry.hash() {
                hashes.insert(name, hash);
            }
        }
    }
    Ok(hashes)
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

fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .with_context(|| format!("无法启动 {program}，请确认已经安装并可在终端中使用"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!("命令执行失败：{}", detail);
    }
    Ok(())
}

fn tool_command(name: &str) -> Command {
    let executable = find_tool(name).unwrap_or_else(|| PathBuf::from(name));
    let mut command = Command::new(&executable);
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
        .context("仓库中没有找到 SKILL.md")
}

fn find_claude_skill(plugin: &str, skill: &str) -> Result<PathBuf> {
    let mut candidates = find_claude_skills(plugin)?
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == skill))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| fs::metadata(path).and_then(|m| m.modified()).ok());
    candidates.pop().context("Claude 插件中没有找到指定技能")
}

fn find_claude_skills(plugin: &str) -> Result<Vec<PathBuf>> {
    let root = dirs::home_dir()
        .context("无法确定用户目录")?
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
        bail!("Claude 插件中没有找到技能");
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
}
