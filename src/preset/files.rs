//! Whole-file preset content. Paths exposed by this API are project-relative.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedFile {
    pub path: PathBuf,
    pub hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    Current,
    Outdated,
    Modified,
    Removed,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileStatus {
    pub path: PathBuf,
    pub state: FileState,
    /// Deleted from the preset; sync releases locally edited copies as well.
    pub removed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConflict {
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileConflictDecision {
    Overwrite,
    Skip,
}

/// Missing entries leave conflicts unresolved. Decisions are per relative path.
pub type FileDecisions = BTreeMap<PathBuf, FileConflictDecision>;

/// Returned through anyhow; callers can downcast to obtain unresolved paths.
#[derive(Debug)]
pub struct PresetFileConflicts {
    pub paths: Vec<PathBuf>,
}

impl std::fmt::Display for PresetFileConflicts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let paths = self
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        f.write_str(&t!("errors.preset_file_conflicts", paths = paths))
    }
}
impl std::error::Error for PresetFileConflicts {}

impl PresetStore {
    /// Sorted relative paths. The special AGENTS.md template is excluded.
    pub fn files(&self, id: &str) -> Result<Vec<PathBuf>> {
        self.preset(id)?;
        let root = self.preset_dir(id).join("files");
        checked_path(&self.preset_dir(id), Path::new("files"))?;
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(&root).follow_links(false) {
            let entry = entry?;
            if entry.file_type().is_dir() {
                continue;
            }
            // Preset paths always use '/', also on Windows.
            let relative = PathBuf::from(
                entry
                    .path()
                    .strip_prefix(&root)?
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
            validate_path(&relative)?;
            checked_path(&root, &relative)?;
            self.read_file(id, &relative)?;
            paths.push(relative);
        }
        paths.sort();
        Ok(paths)
    }

    pub fn read_file(&self, id: &str, path: &Path) -> Result<String> {
        let source = self.file_path(id, path)?;
        let bytes = fs::read(&source)
            .with_context(|| t!("errors.read_failed", path = path.display()).to_string())?;
        text(&bytes, path).map(str::to_owned)
    }

    /// Accept bytes so callers loading a file can receive a localized UTF-8 error.
    pub fn write_file(&mut self, id: &str, path: &Path, content: impl AsRef<[u8]>) -> Result<()> {
        let content = text(content.as_ref(), path)?;
        let path = self.file_path(id, path)?;
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, content)
            .with_context(|| t!("errors.write_failed", path = path.display()).to_string())
    }

    pub fn rename_file(&mut self, id: &str, from: &Path, to: &Path) -> Result<()> {
        let from = self.file_path(id, from)?;
        let to = self.file_path(id, to)?;
        if from == to {
            return Ok(());
        }
        if to.exists() {
            crate::user_bail!("errors.preset_file_exists", path = to.display());
        }
        fs::create_dir_all(to.parent().unwrap())?;
        fs::rename(&from, &to)
            .with_context(|| t!("errors.write_failed", path = to.display()).to_string())
    }

    pub fn delete_file(&mut self, id: &str, path: &Path) -> Result<()> {
        let path = self.file_path(id, path)?;
        fs::remove_file(&path)
            .with_context(|| t!("errors.delete_failed", path = path.display()).to_string())
    }

    fn file_path(&self, id: &str, path: &Path) -> Result<PathBuf> {
        self.preset(id)?;
        validate_path(path)?;
        let root = checked_path(&self.preset_dir(id), Path::new("files"))?;
        checked_path(&root, path)
    }

    /// Check before installation or sync. Managed edits are reported by status,
    /// and require overwrite_modified rather than a conflict decision.
    pub fn file_conflicts(&self, id: &str, project: &Path) -> Result<Vec<FileConflict>> {
        Ok(self
            .file_status(id, project)?
            .into_iter()
            .filter(|f| f.state == FileState::Conflict)
            .map(|f| FileConflict { path: f.path })
            .collect())
    }

    pub(super) fn file_status(&self, id: &str, project: &Path) -> Result<Vec<FileStatus>> {
        let installation = self.installation(id, project);
        let mut status = Vec::new();
        let desired = self.project_files(id, project)?;
        for path in &desired {
            if installation.is_some_and(|i| i.skipped_files.contains(path)) {
                continue;
            }
            let destination = checked_path(project, path)?;
            let current = read_bytes(&destination)?;
            let wanted = file_hash(self.read_file(id, path)?.as_bytes());
            let owned = installation.and_then(|i| i.files.iter().find(|f| f.path == *path));
            let current_hash = current.as_deref().map(file_hash);
            let state = match owned {
                Some(owned) if current_hash.as_ref() != Some(&owned.hash) => FileState::Modified,
                Some(_) if current_hash.as_ref() == Some(&wanted) => FileState::Current,
                None if current_hash.as_ref() == Some(&wanted) => FileState::Outdated,
                Some(_) | None if current.is_none() => FileState::Outdated,
                Some(_) => FileState::Outdated,
                None => FileState::Conflict,
            };
            status.push(FileStatus {
                path: path.clone(),
                state,
                removed: false,
            });
        }
        if let Some(installation) = installation {
            for owned in &installation.files {
                if !desired.contains(&owned.path) {
                    // Even edited removed files need sync to relinquish ownership.
                    let path = checked_path(project, &owned.path)?;
                    let modified =
                        read_bytes(&path)?.is_some_and(|bytes| file_hash(&bytes) != owned.hash);
                    status.push(FileStatus {
                        path: owned.path.clone(),
                        state: if modified {
                            FileState::Modified
                        } else {
                            FileState::Removed
                        },
                        removed: true,
                    });
                }
            }
        }
        Ok(status)
    }

    // Like AGENTS.md, instruction files only go into projects, never the
    // home directory.
    fn project_files(&self, id: &str, project: &Path) -> Result<Vec<PathBuf>> {
        if super::is_home(project) {
            return Ok(Vec::new());
        }
        self.files(id)
    }

    pub(super) fn check_file_decisions(
        &self,
        id: &str,
        project: &Path,
        decisions: &FileDecisions,
    ) -> Result<()> {
        let paths = self
            .file_conflicts(id, project)?
            .into_iter()
            .filter(|f| !decisions.contains_key(&f.path))
            .map(|f| f.path)
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            let conflict = PresetFileConflicts { paths };
            let message = crate::i18n::UserError::new(conflict.to_string());
            return Err(anyhow::Error::new(conflict).context(message));
        }
        Ok(())
    }

    pub(super) fn sync_files(
        &self,
        id: &str,
        installation: &mut PresetInstallation,
        decisions: &FileDecisions,
    ) -> Result<()> {
        let project = &installation.project;
        let desired = self.project_files(id, project)?;
        let removed = installation
            .files
            .iter()
            .filter(|f| !desired.contains(&f.path))
            .cloned()
            .collect::<Vec<_>>();
        release_files(project, &removed)?;
        installation.files.retain(|f| desired.contains(&f.path));
        for path in desired {
            if installation.skipped_files.contains(&path) {
                continue;
            }
            let destination = checked_path(project, &path)?;
            let content = self.read_file(id, &path)?;
            let hash = file_hash(content.as_bytes());
            let current = read_bytes(&destination)?;
            let managed = installation.files.iter().any(|f| f.path == path);
            let conflict = !managed
                && current
                    .as_deref()
                    .is_some_and(|bytes| bytes != content.as_bytes());
            if conflict && decisions.get(&path) == Some(&FileConflictDecision::Skip) {
                installation.skipped_files.push(path);
                continue;
            }
            if current.as_deref() != Some(content.as_bytes()) {
                let mut missing = Vec::new();
                let mut parent = path.parent();
                while let Some(dir) = parent.filter(|p| !p.as_os_str().is_empty()) {
                    if !project.join(dir).exists() {
                        missing.push(dir.to_path_buf());
                    }
                    parent = dir.parent();
                }
                fs::create_dir_all(destination.parent().unwrap())?;
                installation.created_dirs.extend(missing);
                fs::write(&destination, content).with_context(|| {
                    t!("errors.write_failed", path = path.display()).to_string()
                })?;
            }
            installation.files.retain(|f| f.path != path);
            installation.files.push(OwnedFile { path, hash });
        }
        cleanup_dirs(project, &mut installation.created_dirs);
        Ok(())
    }
}

fn validate_path(path: &Path) -> Result<()> {
    let value = path.to_string_lossy();
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|p| p == ".." || p == "." || p.is_empty())
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case(AGENTS_MD))
    {
        crate::user_bail!("errors.preset_file_path", path = path.display());
    }
    Ok(())
}

// Do not follow a project symlink outside the selected project (or a preset
// storage symlink). Normal filesystem errors are still returned to the caller.
fn checked_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                crate::user_bail!("errors.preset_file_symlink", path = relative.display());
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    t!("errors.read_failed", path = relative.display()).to_string()
                });
            }
        }
    }
    Ok(path)
}

fn text<'a>(bytes: &'a [u8], path: &Path) -> Result<&'a str> {
    match std::str::from_utf8(bytes) {
        Ok(value)
            if !value
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) =>
        {
            Ok(value)
        }
        _ => crate::user_bail!("errors.preset_file_text", path = path.display()),
    }
}

fn file_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => {
            Err(e).with_context(|| t!("errors.read_failed", path = path.display()).to_string())
        }
    }
}

pub(super) fn release_files(project: &Path, files: &[OwnedFile]) -> Result<()> {
    for owned in files {
        validate_path(&owned.path)?;
        let path = checked_path(project, &owned.path)?;
        if read_bytes(&path)?.is_some_and(|bytes| file_hash(&bytes) == owned.hash) {
            fs::remove_file(&path).with_context(|| {
                t!("errors.delete_failed", path = owned.path.display()).to_string()
            })?;
        }
    }
    Ok(())
}

pub(super) fn cleanup_dirs(project: &Path, dirs: &mut Vec<PathBuf>) {
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    dirs.dedup();
    dirs.retain(|relative| {
        if validate_path(relative).is_err() {
            return true;
        }
        let Ok(path) = checked_path(project, relative) else {
            return true;
        };
        match fs::remove_dir(path) {
            Ok(()) => false,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true, // Nonempty directories belong to the user too.
        }
    });
}
