//! One-shot discovery and adoption. Scanning never installs packages or changes sources.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{SkillOrigin, SkillRecord, SkillSummary, directory_link, library};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReferenceKind {
    Link,
    /// A directly installed real directory, observed but never replaced.
    Direct,
    /// An ancestor is a directory alias; never replace the real child directory.
    Alias,
    /// A native plugin contribution is not a standalone installation link.
    Plugin,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillReference {
    pub path: PathBuf,
    pub source: PathBuf,
    pub kind: ReferenceKind,
    pub original_target: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct AdoptionCandidate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: PathBuf,
    pub origin: SkillOrigin,
    pub references: Vec<SkillReference>,
    pub issue: Option<String>,
    pub existing_storage: Option<String>,
    marker_hash: u64,
}

impl AdoptionCandidate {
    #[cfg(test)]
    pub(crate) fn fixture(id: &str, origin: SkillOrigin) -> Self {
        Self {
            id: id.into(),
            name: "same-name".into(),
            description: String::new(),
            source: PathBuf::from(id),
            origin,
            references: vec![],
            issue: None,
            existing_storage: None,
            marker_hash: 0,
        }
    }
    pub fn identity(&self) -> String {
        self.origin.identity_key(&self.name)
    }
    pub fn record(&self) -> SkillRecord {
        SkillRecord {
            name: self.name.clone(),
            storage_name: String::new(),
            description: self.description.clone(),
            origin: self.origin.clone(),
            update_available: false,
            group_id: None,
            last_operated_at: 0,
        }
    }
    pub fn verify(&self) -> Result<()> {
        if marker_hash(&self.source)? != self.marker_hash {
            bail!("技能已变化，请重新扫描：{}", self.source.display());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct AdoptionScan {
    pub candidates: Vec<AdoptionCandidate>,
    identities: HashMap<String, Vec<usize>>,
    by_id: HashMap<String, usize>,
    selectable: HashSet<String>,
    /// Kept for diagnostics, not presented as a noisy scan counter.
    pub skipped: Vec<PathBuf>,
}

impl AdoptionScan {
    pub fn new(candidates: Vec<AdoptionCandidate>, skipped: Vec<PathBuf>) -> Self {
        let mut scan = Self {
            candidates,
            skipped,
            ..Self::default()
        };
        scan.reindex();
        scan
    }
    fn reindex(&mut self) {
        self.identities.clear();
        self.by_id.clear();
        for (index, candidate) in self.candidates.iter().enumerate() {
            self.identities
                .entry(candidate.identity())
                .or_default()
                .push(index);
            self.by_id.insert(candidate.id.clone(), index);
        }
        self.selectable = self
            .candidates
            .iter()
            .filter(|c| c.issue.is_none() && !self.has_conflict(&c.identity()))
            .map(|c| c.id.clone())
            .collect();
    }
    pub fn retain(&mut self, keep: impl FnMut(&AdoptionCandidate) -> bool) {
        self.candidates.retain(keep);
        self.reindex();
    }
    pub fn has_conflict(&self, identity: &str) -> bool {
        self.identities
            .get(identity)
            .is_some_and(|items| items.len() > 1)
    }
    pub fn selectable_ids(&self) -> &HashSet<String> {
        &self.selectable
    }
    pub fn contains_id(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }
    pub fn default_selection(&self) -> HashSet<String> {
        self.selectable.clone()
    }
    pub fn variants(
        &self,
        candidate: &AdoptionCandidate,
    ) -> impl Iterator<Item = &AdoptionCandidate> {
        self.identities
            .get(&candidate.identity())
            .into_iter()
            .flatten()
            .map(|index| &self.candidates[*index])
    }
    pub fn select(&self, selected: &mut HashSet<String>, id: &str) {
        let Some(&index) = self.by_id.get(id) else {
            return;
        };
        let candidate = &self.candidates[index];
        if candidate.issue.is_some() || selected.remove(id) {
            return;
        }
        for sibling in self.variants(candidate) {
            selected.remove(&sibling.id);
        }
        selected.insert(id.to_owned());
    }
    pub fn references_for(&self, candidate: &AdoptionCandidate) -> Vec<SkillReference> {
        let mut seen = HashSet::new();
        self.variants(candidate)
            .flat_map(|c| c.references.iter())
            .filter(|r| seen.insert(crate::project::installation_key(&r.path)))
            .cloned()
            .collect()
    }
}
fn marker_hash(path: &Path) -> Result<u64> {
    let content = fs::read(path.join("SKILL.md"))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    Ok(hasher.finish())
}

fn read_marker(path: &Path) -> Result<(String, String)> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("缺少 frontmatter");
    }
    let mut header = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        header.push(line);
    }
    if !closed {
        bail!("frontmatter 未闭合");
    }
    #[derive(Deserialize)]
    struct Metadata {
        name: String,
        #[serde(default)]
        description: String,
    }
    let metadata: Metadata = serde_yaml::from_str(&header.join("\n"))?;
    library::validate_name(&metadata.name)?;
    if metadata.name.trim().is_empty() {
        bail!("技能名称为空");
    }
    Ok((metadata.name, metadata.description))
}

/// Privacy exclusions are applied before opening a directory, including symlink targets.
pub fn excluded(path: &Path, home: &Path, library_dir: &Path) -> bool {
    if path.starts_with(library_dir) {
        return true;
    }
    if privacy_excluded(path, home) {
        return true;
    }
    for suffix in [
        ".claude/plugins",
        ".codex/plugins",
        ".codex/sessions",
        ".codex/memories",
        ".codex/skills/.system",
        ".npm",
        ".cache",
        ".rustup",
        ".cargo/registry",
        ".cargo/git",
        ".pnpm-store",
        ".bun/install/cache",
    ] {
        if path.starts_with(home.join(suffix)) {
            return true;
        }
    }
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git"
                | ".venv"
                | "venv"
                | "__pycache__"
                | ".next"
                | ".nuxt"
                | "node_modules"
                | "target"
                | "build"
                | "dist"
                | "DerivedData"
                | "Caches"
                | ".Trash"
        )
    })
}

fn privacy_excluded(path: &Path, home: &Path) -> bool {
    for name in [
        "Pictures",
        "Music",
        "Movies",
        "Library",
        ".Trash",
        "Applications",
    ] {
        if path.starts_with(home.join(name)) {
            return true;
        }
    }
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        [
            ".photoslibrary",
            ".photolibrary",
            ".musiclibrary",
            ".imovielibrary",
            ".app",
        ]
        .iter()
        .any(|ext| name.to_ascii_lowercase().ends_with(ext))
    })
}

fn json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn installed_location(path: &Path) -> bool {
    path.parent()
        .is_some_and(|p| p.file_name().is_some_and(|n| n == "skills"))
}

fn npx_origin(
    path: &Path,
    name: &str,
    home: &Path,
    cache: &mut HashMap<PathBuf, Option<serde_json::Value>>,
) -> Option<SkillOrigin> {
    // Do not associate a random local folder with a same-named global lock entry.
    if !installed_location(path) {
        return None;
    }
    let install_name = path.file_name()?.to_str()?;
    for root in path.ancestors().skip(1) {
        for lock_path in [
            root.join("skills-lock.json"),
            root.join(".agents/.skill-lock.json"),
        ] {
            let value = cache
                .entry(lock_path.clone())
                .or_insert_with(|| json(&lock_path));
            let Some(entry) = value
                .as_ref()
                .and_then(|v| v.get("skills"))
                .and_then(|v| v.get(install_name).or_else(|| v.get(name)))
            else {
                continue;
            };
            let kind = entry
                .get("sourceType")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if matches!(kind, "local" | "node_modules") {
                continue;
            }
            let repository = entry
                .get("source")
                .or_else(|| entry.get("sourceUrl"))
                .and_then(|v| v.as_str())?;
            if repository.is_empty() {
                continue;
            }
            return Some(SkillOrigin::Npx {
                repository: repository.to_string(),
                skill: name.into(),
                source_hash: entry
                    .get("skillFolderHash")
                    .or_else(|| entry.get("computedHash"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
        if root == home {
            break;
        }
    }
    None
}

fn git_origin(path: &Path) -> Option<SkillOrigin> {
    for root in path.ancestors() {
        if !root.join(".git").exists() {
            continue;
        }
        let relative = path.strip_prefix(root).ok()?;
        // The consuming project's remote is not the upstream of installed copies.
        if relative.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some(".agents" | ".claude" | ".codex" | ".cursor" | ".pi")
            )
        }) {
            return None;
        }
        let output = Command::new("git")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .arg("-C")
            .arg(root)
            .args(["config", "--get", "remote.origin.url"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let repository = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if repository.is_empty() {
            return None;
        }
        return Some(SkillOrigin::Git {
            repository,
            subdir: (!relative.as_os_str().is_empty())
                .then(|| relative.to_string_lossy().into_owned()),
        });
    }
    None
}

/// Only direct install roots count; plugin registries and extension manifests
/// are deliberately not read. Keep project roots aligned with normal installation.
const EXTRA_DIRECT_ROOTS: &[&str] = &[
    ".pi/agent/skills",
    ".config/opencode/skills",
    ".openclaw/skills",
    ".hermes/skills",
    ".copilot/skills",
    ".gemini/config/skills",
    ".config/agents/skills",
    ".config/amp/skills",
];

fn skill_root(path: &Path) -> bool {
    crate::agents::PROJECT_INSTALL_TARGETS
        .iter()
        .any(|target| path.ends_with(crate::agents::target_directory(*target)))
        || EXTRA_DIRECT_ROOTS
            .iter()
            .any(|suffix| path.ends_with(suffix))
}

fn agent_directory(path: &Path) -> bool {
    crate::agents::PROJECT_INSTALL_TARGETS.iter().any(|target| {
        Path::new(crate::agents::target_directory(*target))
            .parent()
            .is_some_and(|parent| path.ends_with(parent))
    }) || EXTRA_DIRECT_ROOTS.iter().any(|root| {
        Path::new(root)
            .parent()
            .is_some_and(|parent| path.ends_with(parent))
    })
}

fn plugin_path(path: &Path) -> bool {
    // Applies to project-local caches as well as global ones, including link targets.
    let components = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    components.windows(2).any(|pair| {
        matches!(
            pair[0].as_ref(),
            ".claude" | ".codex" | ".cursor" | ".opencode" | ".pi"
        ) && matches!(pair[1].as_ref(), "plugins" | "extensions" | "packages")
    }) || components.windows(3).any(|part| {
        (part[0] == ".pi" && part[1] == "agent" || part[0] == ".config" && part[1] == "opencode")
            && matches!(part[2].as_ref(), "plugins" | "extensions" | "packages")
    })
}

struct Scanner<'a> {
    home: &'a Path,
    library: &'a Path,
    cancel: &'a AtomicBool,
    candidates: BTreeMap<PathBuf, AdoptionCandidate>,
    deferred_links: Vec<SkillReference>,
    locks: HashMap<PathBuf, Option<serde_json::Value>>,
    skipped: Vec<PathBuf>,
    visited: HashSet<PathBuf>,
}

impl Scanner<'_> {
    fn add(&mut self, entry: &Path) {
        if self.cancel.load(Ordering::Relaxed) {
            return;
        }
        let Ok(source) = entry.canonicalize() else {
            return;
        };
        if plugin_path(&source) {
            return;
        }
        let link = directory_link::inspect(entry).ok().flatten();
        if source.starts_with(self.library) {
            if let Some(link) = link {
                self.deferred_links.push(SkillReference {
                    path: entry.into(),
                    source,
                    kind: ReferenceKind::Link,
                    original_target: Some(link.target),
                });
            }
            return;
        }
        if excluded(&source, self.home, self.library) {
            return;
        }
        let Ok((name, description)) = read_marker(&source.join("SKILL.md")) else {
            return;
        };
        let Ok(hash) = marker_hash(&source) else {
            return;
        };
        let origin = npx_origin(entry, &name, self.home, &mut self.locks)
            .or_else(|| npx_origin(&source, &name, self.home, &mut self.locks))
            .unwrap_or_else(|| SkillOrigin::Local {
                path: source.clone(),
                source_root: None,
            });
        let reference = SkillReference {
            path: entry.into(),
            source: source.clone(),
            kind: if link.is_some() {
                ReferenceKind::Link
            } else if crate::project::installation_key(entry) != entry {
                ReferenceKind::Alias
            } else {
                ReferenceKind::Direct
            },
            original_target: link.map(|link| link.target),
        };
        let candidate =
            self.candidates
                .entry(source.clone())
                .or_insert_with(|| AdoptionCandidate {
                    id: source.to_string_lossy().into_owned(),
                    name: name.clone(),
                    description,
                    source,
                    origin: origin.clone(),
                    references: Vec::new(),
                    issue: None,
                    existing_storage: None,
                    marker_hash: hash,
                });
        if !matches!(origin, SkillOrigin::Local { .. }) {
            if matches!(candidate.origin, SkillOrigin::Local { .. }) {
                candidate.origin = origin;
            } else if candidate.origin.identity_key(&name) != origin.identity_key(&name) {
                candidate.issue = Some("来源记录不一致".into());
            }
        }
        if !candidate
            .references
            .iter()
            .any(|r| r.path == reference.path)
        {
            candidate.references.push(reference);
        }
    }

    fn direct_root(&mut self, root: &Path) {
        let Ok(target) = root.canonicalize() else {
            return;
        };
        if excluded(&target, self.home, self.library) || plugin_path(&target) {
            return;
        }
        // Deduplicate logical roots, not physical ones: root aliases are references too.
        if !self.visited.insert(root.to_path_buf()) {
            return;
        }
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => {
                self.skipped.push(root.into());
                return;
            }
        };
        for entry in entries {
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            match entry {
                Ok(entry) => self.add(&entry.path()),
                Err(_) => self.skipped.push(root.into()),
            }
        }
    }

    fn walk(&mut self, root: &Path) {
        let mut walker = WalkDir::new(root)
            .follow_links(false)
            .max_open(32)
            .into_iter();
        while let Some(next) = walker.next() {
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            let entry = match next {
                Ok(entry) => entry,
                Err(error) => {
                    if let Some(path) = error.path() {
                        self.skipped.push(path.into());
                    }
                    continue;
                }
            };
            let path = entry.path();
            if excluded(path, self.home, self.library) || plugin_path(path) {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            if skill_root(path) {
                self.direct_root(path);
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
            } else if agent_directory(path) {
                self.direct_root(&path.join("skills"));
                if path.ends_with(".pi") {
                    self.direct_root(&path.join("agent/skills"));
                }
                // Do not walk sessions, plugins, extensions, or skill payloads.
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
            }
        }
    }
}
pub fn scan(
    home: &Path,
    projects: &[PathBuf],
    library_dir: &Path,
    managed: &[SkillSummary],
    cancel: &AtomicBool,
) -> Result<AdoptionScan> {
    let mut roots = vec![home.to_path_buf()];
    roots.extend_from_slice(projects);
    scan_roots(home, &roots, library_dir, managed, cancel)
}

pub fn scan_roots(
    home: &Path,
    roots: &[PathBuf],
    library_dir: &Path,
    managed: &[SkillSummary],
    cancel: &AtomicBool,
) -> Result<AdoptionScan> {
    let home = home.canonicalize().context("找不到用户目录")?;
    let canonical_library = library_dir
        .canonicalize()
        .unwrap_or_else(|_| library_dir.to_path_buf());
    let roots = roots
        .iter()
        .filter(|root| root.exists())
        .cloned()
        .collect::<Vec<_>>();
    let mut scanner = Scanner {
        home: &home,
        library: &canonical_library,
        cancel,
        candidates: BTreeMap::new(),
        deferred_links: Vec::new(),
        locks: HashMap::new(),
        skipped: Vec::new(),
        visited: HashSet::new(),
    };
    for project in &roots {
        scanner.walk(project);
    }
    if cancel.load(Ordering::Relaxed) {
        bail!("已取消扫描");
    }
    for candidate in scanner.candidates.values_mut() {
        if cancel.load(Ordering::Relaxed) {
            bail!("已取消扫描");
        }
        if matches!(candidate.origin, SkillOrigin::Local { .. }) {
            if let Some(origin) = git_origin(&candidate.source) {
                candidate.origin = origin;
            }
        }
    }
    // Existing library identities are reusable; do not collapse same-named different sources.
    for skill in managed {
        let Ok(source) = skill.path.canonicalize() else {
            continue;
        };
        if let Some(candidate) = scanner.candidates.get_mut(&source) {
            candidate.origin = skill.record.origin.clone();
            candidate.existing_storage = Some(skill.record.storage_name.clone());
        } else if scanner
            .candidates
            .values()
            .any(|c| c.identity() == skill.record.identity_key())
        {
            if let Ok(hash) = marker_hash(&source) {
                scanner.candidates.insert(
                    source.clone(),
                    AdoptionCandidate {
                        id: source.to_string_lossy().into_owned(),
                        name: skill.record.name.clone(),
                        description: skill.record.description.clone(),
                        source,
                        origin: skill.record.origin.clone(),
                        references: Vec::new(),
                        issue: None,
                        existing_storage: Some(skill.record.storage_name.clone()),
                        marker_hash: hash,
                    },
                );
            }
        }
    }
    for reference in scanner.deferred_links {
        if let Some(candidate) = scanner.candidates.get_mut(&reference.source) {
            if !candidate
                .references
                .iter()
                .any(|r| r.path == reference.path)
            {
                candidate.references.push(reference);
            }
        }
    }
    let mut candidates = scanner.candidates.into_values().collect::<Vec<_>>();
    candidates.sort_by_key(|c| (c.origin.source().key(), c.name.clone(), c.source.clone()));
    scanner.skipped.sort();
    scanner.skipped.dedup();
    Ok(AdoptionScan::new(candidates, scanner.skipped))
}

/// Snapshot guards are checked again immediately before replacing any link.
pub fn validate_reference(reference: &SkillReference) -> Result<()> {
    if reference.kind != ReferenceKind::Link {
        return Ok(());
    }
    let link = directory_link::inspect(&reference.path)?.context("引用已变化，请重新扫描")?;
    if Some(&link.target) != reference.original_target.as_ref()
        || reference.path.canonicalize()? != reference.source
    {
        bail!("引用已变化，请重新扫描：{}", reference.path.display());
    }
    Ok(())
}

pub fn replace_link(path: &Path, target: &Path) -> Result<()> {
    let parent = path.parent().context("无效的安装路径")?;
    let temp = tempfile::Builder::new()
        .prefix(".kitter-adopt-")
        .tempdir_in(parent)?;
    let staged = temp.path().join("link");
    directory_link::create(target, &staged)?;
    fs::rename(&staged, path).with_context(|| format!("无法切换引用：{}", path.display()))
}

pub fn rollback_links(changed: &[SkillReference]) -> Vec<String> {
    changed
        .iter()
        .rev()
        .filter_map(|r| {
            r.original_target
                .as_ref()
                .and_then(|target| replace_link(&r.path, target).err())
                .map(|e| e.to_string())
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn skill(path: &Path, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: fixture\n---\nbody"),
        )
        .unwrap();
    }
    fn scan_home(home: &Path) -> AdoptionScan {
        scan(
            home,
            &[],
            &home.join("kitter-library"),
            &[],
            &AtomicBool::new(false),
        )
        .unwrap()
    }
    #[test]
    fn marker_not_directory_name_defines_identity_and_aliases_are_merged() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let source = home.join("sources/not-the-name");
        skill(&source, "actual-name");
        fs::create_dir_all(home.join(".agents/skills")).unwrap();
        fs::create_dir_all(home.join("project/.claude/skills")).unwrap();
        symlink(&source, home.join(".agents/skills/alias-one")).unwrap();
        symlink(
            "../../../sources/not-the-name",
            home.join("project/.claude/skills/alias-two"),
        )
        .unwrap();
        fs::create_dir_all(home.join("looks-like-a-skill")).unwrap();
        fs::write(home.join("looks-like-a-skill/SKILL.md"), "# no metadata").unwrap();
        let scan = scan_home(&home);
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].name, "actual-name");
        assert_eq!(scan.candidates[0].references.len(), 2);
    }
    #[test]
    fn same_name_different_local_sources_coexist() {
        let temp = tempfile::tempdir().unwrap();
        skill(&temp.path().join(".agents/skills/a"), "same");
        skill(&temp.path().join(".agents/skills/b"), "same");
        let scan = scan_home(temp.path());
        assert_eq!(scan.candidates.len(), 2);
        assert_eq!(scan.default_selection().len(), 2);
        assert_ne!(scan.candidates[0].identity(), scan.candidates[1].identity());
    }
    #[test]
    fn only_direct_agent_installations_are_discovered() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let roots = [
            ".agents/skills",
            ".claude/skills",
            ".codex/skills",
            ".pi/skills",
            ".pi/agent/skills",
            ".config/opencode/skills",
            "project/.cursor/skills",
        ];
        for (index, root) in roots.iter().enumerate() {
            skill(&home.join(root).join("folder"), &format!("direct-{index}"));
            skill(
                &home.join(root).join("folder/examples/nested"),
                "nested-example",
            );
            skill(
                &home.join(root).join("collection/inside"),
                "nested-collection",
            );
        }
        for path in [
            "checkout/skills/loose",
            "project/docs/example",
            ".pi/agent/extensions/pkg/.claude/skills/hidden",
            ".codex/plugins/cache/pkg/.agents/skills/hidden",
        ] {
            skill(&home.join(path), "not-direct");
        }
        let scan = scan_home(&home);
        assert_eq!(scan.candidates.len(), roots.len());
        assert!(
            scan.candidates
                .iter()
                .all(|c| c.name.starts_with("direct-"))
        );
    }

    #[test]
    fn selecting_an_aliased_skills_root_keeps_its_logical_installation_paths() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        skill(&home.join("source/demo"), "demo");
        fs::create_dir_all(home.join(".claude")).unwrap();
        symlink(home.join("source"), home.join(".claude/skills")).unwrap();
        let scan = scan_roots(
            &home,
            &[home.join(".claude/skills")],
            &home.join("library"),
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(
            scan.candidates[0].references[0].path,
            home.join(".claude/skills/demo")
        );
        assert_eq!(scan.candidates[0].references[0].kind, ReferenceKind::Alias);
    }

    #[test]
    fn large_selection_uses_identity_indexes_and_reindexes_after_filtering() {
        let origin = SkillOrigin::Npx {
            repository: "fixture/repo".into(),
            skill: "same-name".into(),
            source_hash: None,
        };
        let candidates = (0..10_000)
            .map(|index| {
                let path = PathBuf::from(format!("/fixture/{index}"));
                AdoptionCandidate::fixture(
                    path.to_str().unwrap(),
                    if index < 2 {
                        origin.clone()
                    } else {
                        SkillOrigin::Local {
                            path: path.clone(),
                            source_root: None,
                        }
                    },
                )
            })
            .collect();
        let mut scan = AdoptionScan::new(candidates, vec![]);
        assert_eq!(scan.selectable_ids().len(), 9_998);
        let mut selected = scan.default_selection();
        scan.select(&mut selected, "/fixture/0");
        scan.select(&mut selected, "/fixture/1");
        assert!(!selected.contains("/fixture/0"));
        assert!(selected.contains("/fixture/1"));
        scan.retain(|candidate| candidate.id != "/fixture/0");
        assert_eq!(scan.selectable_ids().len(), 9_999);
        assert!(!scan.has_conflict(&origin.identity_key("same-name")));
    }
    #[test]
    fn git_source_records_the_repository_and_skill_subdirectory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("checkout");
        skill(&root.join("skills/not-the-name"), "review");
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .arg(&root)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&root)
                .args([
                    "config",
                    "remote.origin.url",
                    "https://github.com/fixture/skills.git"
                ])
                .status()
                .unwrap()
                .success()
        );
        fs::create_dir_all(temp.path().join(".agents/skills")).unwrap();
        symlink(
            root.join("skills/not-the-name"),
            temp.path().join(".agents/skills/review"),
        )
        .unwrap();
        let scan = scan_home(temp.path());
        assert_eq!(scan.candidates.len(), 1);
        assert!(
            matches!(&scan.candidates[0].origin, SkillOrigin::Git { repository, subdir } if repository == "https://github.com/fixture/skills.git" && subdir.as_deref() == Some("skills/not-the-name"))
        );
    }
    #[test]
    fn same_npx_identity_requires_choice_and_keeps_all_references() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        for project in ["a", "b"] {
            let root = home.join(project);
            skill(&root.join(".agents/skills/demo"), "demo");
            fs::create_dir_all(root.join(".claude/skills")).unwrap();
            symlink(
                "../../.agents/skills/demo",
                root.join(".claude/skills/demo"),
            )
            .unwrap();
            fs::write(root.join("skills-lock.json"), r#"{"version":1,"skills":{"demo":{"source":"owner/repo","sourceType":"github","computedHash":"x"}}}"#).unwrap();
        }
        let scan = scan_home(&home);
        assert_eq!(scan.candidates.len(), 2);
        assert!(scan.default_selection().is_empty());
        let mut selected = HashSet::new();
        scan.select(&mut selected, &scan.candidates[0].id);
        scan.select(&mut selected, &scan.candidates[1].id);
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&scan.candidates[1].id));
        assert_eq!(scan.references_for(&scan.candidates[1]).len(), 4);
    }
    #[test]
    fn excludes_media_packages_caches_and_does_not_follow_privacy_aliases() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        for name in [
            "Pictures/photo",
            "Music/music",
            "Library/private",
            "stuff/foo.photoslibrary/hidden",
            "node_modules/package",
            "target/debug",
            "kitter-library/owned",
            ".codex/skills/.system/builtin",
        ] {
            skill(&home.join(name), "hidden");
        }
        symlink(home.join("Pictures/photo"), home.join("innocent-link")).unwrap();
        skill(&home.join(".claude/skills/visible"), "visible");
        let scan = scan_home(&home);
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].name, "visible");
    }
    #[test]
    fn parent_alias_is_an_observation_not_a_replaceable_directory() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        skill(&home.join(".agents/skills/demo"), "demo");
        fs::create_dir_all(home.join(".claude")).unwrap();
        symlink("../.agents/skills", home.join(".claude/skills")).unwrap();
        let scan = scan_home(&home);
        assert_eq!(scan.candidates.len(), 1);
        assert!(
            scan.candidates[0]
                .references
                .iter()
                .any(|r| r.kind == ReferenceKind::Alias)
        );
    }
    #[test]
    fn plugin_registries_caches_and_direct_links_into_plugins_are_excluded() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let root = home.join(".claude/plugins/cache/market/plugin/v2");
        skill(&root.join("skills/wrong-dir"), "actual");
        skill(
            &home.join(".claude/plugins/cache/market/plugin/v1/skills/old"),
            "stale",
        );
        fs::write(home.join(".claude/plugins/installed_plugins.json"), serde_json::to_vec(&serde_json::json!({"plugins":{"plugin@market":[{"scope":"user","installPath":root}]}})).unwrap()).unwrap();
        fs::create_dir_all(home.join(".codex/skills")).unwrap();
        symlink(
            root.join("skills/wrong-dir"),
            home.join(".codex/skills/external-plugin-link"),
        )
        .unwrap();
        symlink(
            home.join(".claude/plugins/cache/market/plugin/v1/skills/old"),
            home.join("stale-plugin-link"),
        )
        .unwrap();
        let scan = scan_home(&home);
        assert!(scan.candidates.is_empty());
    }
    #[test]
    fn invalid_markers_never_fall_back_to_directory_names() {
        let temp = tempfile::tempdir().unwrap();
        for (index, marker) in [
            "---\ndescription: missing name\n---",
            "---\nname: unclosed",
            "---\nname: ../escape\n---",
            "---\nname: [invalid]\n---",
        ]
        .iter()
        .enumerate()
        {
            let path = temp.path().join(format!(".agents/skills/folder-{index}"));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("SKILL.md"), marker).unwrap();
        }
        assert!(scan_home(temp.path()).candidates.is_empty());
    }
    #[test]
    fn cancellation_and_marker_drift_are_detected() {
        let temp = tempfile::tempdir().unwrap();
        skill(&temp.path().join(".agents/skills/demo"), "demo");
        let scan = scan_home(temp.path());
        fs::write(temp.path().join(".agents/skills/demo/SKILL.md"), "changed").unwrap();
        assert!(scan.candidates[0].verify().is_err());
        assert!(
            super::scan(
                temp.path(),
                &[],
                &temp.path().join("lib"),
                &[],
                &AtomicBool::new(true)
            )
            .is_err()
        );
    }
    #[test]
    fn stale_link_is_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        skill(&home.join("source"), "demo");
        skill(&home.join("other"), "other");
        fs::create_dir_all(home.join(".agents/skills")).unwrap();
        symlink(home.join("source"), home.join(".agents/skills/link")).unwrap();
        let scan = scan_home(&home);
        let reference = &scan
            .candidates
            .iter()
            .find(|c| c.name == "demo")
            .unwrap()
            .references[0];
        replace_link(&home.join(".agents/skills/link"), &home.join("other")).unwrap();
        assert!(validate_reference(reference).is_err());
        assert_eq!(
            home.join(".agents/skills/link").canonicalize().unwrap(),
            home.join("other")
        );
    }
}
