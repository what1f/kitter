//! Choose where Kitter keeps its data: move the current data into an empty
//! folder, or switch to a folder that already holds Kitter data.
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{
    AppConfig, SkillLibrary,
    agent_plugins::AgentRegistry,
    config::{self, LOCATION_FILE},
    directory_link,
    preset::PresetStore,
};

/// Regenerated on demand, so a move leaves them behind.
const CACHES: [&str; 4] = [
    "npx-sources",
    "update-cache",
    "update-check.lock",
    "crash.log",
];
/// Folder noise that does not make a folder "not empty".
const SYSTEM_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFolder {
    /// Empty or missing: the current data moves there.
    Empty,
    /// Already holds Kitter data: Kitter switches to it and leaves the current data.
    Existing,
}

#[derive(Debug, Default)]
pub struct MoveReport {
    /// Installations that still point at the old folder.
    pub stale_links: Vec<PathBuf>,
}

pub fn current() -> PathBuf {
    config::app_data_dir()
}

pub fn inspect(target: &Path) -> Result<DataFolder> {
    ensure_configurable()?;
    inspect_in(&current(), &config::default_data_dir(), target)
}

/// Move into an empty folder, or switch to existing data. Takes effect in this
/// process immediately; other running Kitter processes need a restart.
pub fn change(target: &Path) -> Result<(DataFolder, MoveReport)> {
    ensure_configurable()?;
    let current = current();
    let default = config::default_data_dir();
    // Spell the default folder one way so the pointer file gets removed.
    let target = if resolve(target) == resolve(&default) {
        default.as_path()
    } else {
        target
    };
    let kind = inspect_in(&current, &default, target)?;
    let report = match kind {
        DataFolder::Existing => {
            config::write_location(&default, target)?;
            MoveReport::default()
        }
        DataFolder::Empty => {
            let roots = install_roots(&current)?;
            move_in(&current, &default, &roots, target)?
        }
    };
    config::set_app_data_dir(target);
    AgentRegistry::reload();
    Ok((kind, report))
}

fn ensure_configurable() -> Result<()> {
    if std::env::var_os("KITTER_HOME").is_some() {
        crate::user_bail!("errors.data_location_env");
    }
    Ok(())
}

fn inspect_in(current: &Path, default: &Path, target: &Path) -> Result<DataFolder> {
    if !target.is_absolute() {
        crate::user_bail!("errors.absolute_path_required");
    }
    let (current, target_resolved) = (resolve(current), resolve(target));
    if current == target_resolved {
        crate::user_bail!("errors.data_location_same");
    }
    if target_resolved.starts_with(&current) || current.starts_with(&target_resolved) {
        crate::user_bail!("errors.data_location_nested");
    }
    let is_default = target_resolved == resolve(default);
    let entries = match fs::read_dir(target) {
        Ok(entries) => entries
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| {
                !SYSTEM_FILES.contains(&name.as_str())
                    // The default folder keeps the pointer and the Windows CLI.
                    && !(is_default && (name == LOCATION_FILE || name == "bin"))
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if entries.is_empty() {
        Ok(DataFolder::Empty)
    } else if entries
        .iter()
        .any(|name| name == "config.json" || name == "registry.json")
    {
        Ok(DataFolder::Existing)
    } else {
        crate::user_bail!("errors.data_location_not_empty")
    }
}

/// Canonical path, also for folders that do not exist yet.
fn resolve(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(canonical, |path, part| path.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_owned());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Every project Kitter knows, plus the home folder for user-level installs.
fn install_roots(data_dir: &Path) -> Result<Vec<PathBuf>> {
    let config = AppConfig::load_from(data_dir)?;
    let mut roots = dirs::home_dir().into_iter().collect::<Vec<_>>();
    roots.extend(config.recent_projects);
    roots.extend(PresetStore::open_in(data_dir)?.projects());
    Ok(roots)
}

fn move_in(current: &Path, default: &Path, roots: &[PathBuf], target: &Path) -> Result<MoveReport> {
    let library = SkillLibrary::open_in(current)?;
    let library_dir = library.config.library_dir.clone();
    let own_library = library_dir == current.join("skills");
    let links = managed_links(roots, &library_dir);

    let created = !target.exists();
    fs::create_dir_all(target)?;
    let mut copied = Vec::new();
    let copy = (|| -> Result<()> {
        for name in entries(current, current == default)? {
            if !own_library && name == "skills" {
                continue;
            }
            copied.push(target.join(&name));
            copy_entry(&current.join(&name), &target.join(&name))?;
        }
        if !own_library {
            copied.push(target.join("skills"));
            copy_entry(&library_dir, &target.join("skills"))?;
        }
        let mut config = AppConfig::load_from(target)?;
        config.library_dir = target.join("skills");
        config.library_fallback = None;
        config.save_to(target)?;
        config::write_location(default, target)
    })();
    if let Err(error) = copy {
        // Nothing points at the copy yet; remove it and keep using the current data.
        if created {
            let _ = remove_entry(target);
        } else {
            for path in copied {
                let _ = remove_entry(&path);
            }
        }
        return Err(error);
    }

    let new_library = target.join("skills");
    let mut report = MoveReport::default();
    for (link, relative) in links {
        let relinked = directory_link::inspect(&link)
            .ok()
            .flatten()
            .is_some_and(|old| {
                directory_link::remove(&link, old).is_ok()
                    && directory_link::create(&new_library.join(&relative), &link).is_ok()
            });
        if !relinked {
            report.stale_links.push(link);
        }
    }

    // The data is safe in its new place; leftovers here are only clutter.
    // A library folder outside the data folder was chosen by hand, so keep it.
    for name in entries(current, current == default)?
        .into_iter()
        .chain(CACHES.iter().map(|name| (*name).to_owned()))
    {
        let _ = remove_entry(&current.join(name));
    }
    if current != default {
        let _ = fs::remove_dir(current);
    }
    Ok(report)
}

/// Data entries to move. The default folder also keeps the pointer and CLI.
fn entries(dir: &Path, is_default: bool) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if CACHES.contains(&name.as_str())
            || SYSTEM_FILES.contains(&name.as_str())
            || (is_default && (name == LOCATION_FILE || name == "bin"))
        {
            continue;
        }
        names.push(name);
    }
    Ok(names)
}

/// Links in agent skill folders that point into the library, with the path
/// below the library they point at.
fn managed_links(roots: &[PathBuf], library_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let canonical_library = library_dir.canonicalize().ok();
    let below_library = |target: &Path| -> Option<PathBuf> {
        if let Ok(relative) = target.strip_prefix(library_dir) {
            return Some(relative.to_path_buf());
        }
        let canonical_library = canonical_library.as_deref()?;
        if let Ok(relative) = target.strip_prefix(canonical_library) {
            return Some(relative.to_path_buf());
        }
        // A linked source resolves outside the library; compare its parent.
        (target.parent()?.canonicalize().ok()? == canonical_library)
            .then(|| target.file_name().map(PathBuf::from))
            .flatten()
    };
    let targets = AgentRegistry::global().install_targets();
    let mut folders = HashSet::new();
    let mut links = Vec::new();
    for root in roots {
        for target in &targets {
            let Some(folder) = crate::agents::installation_root(root, target) else {
                continue;
            };
            if !folders.insert(folder.clone()) {
                continue;
            }
            for entry in fs::read_dir(&folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if let Some(link) = directory_link::inspect(&path).ok().flatten()
                    && let Some(relative) = below_library(&link.target)
                    && !relative.as_os_str().is_empty()
                {
                    links.push((path, relative));
                }
            }
        }
    }
    links
}

/// Copy a tree, recreating links instead of following them.
fn copy_entry(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    #[cfg(windows)]
    if let Some(link) = directory_link::inspect(source)?
        && link.kind == directory_link::DirectoryLinkKind::Junction
    {
        directory_link::create(&link.target, destination)?;
        return Ok(());
    }
    if metadata.file_type().is_symlink() {
        // Keep relative targets relative so links inside the data still resolve.
        let raw = fs::read_link(source)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&raw, destination)?;
        #[cfg(windows)]
        {
            let created = if source.is_dir() {
                std::os::windows::fs::symlink_dir(&raw, destination).or_else(|_| {
                    let link = directory_link::inspect(source)?.ok_or(io::ErrorKind::NotFound)?;
                    directory_link::create(&link.target, destination)
                })
            } else {
                std::os::windows::fs::symlink_file(&raw, destination)
                    .or_else(|_| fs::copy(source, destination).map(|_| ()))
            };
            created?;
        }
    } else if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, destination)?;
    }
    Ok(())
}

/// Remove a tree without following links out of it.
fn remove_entry(path: &Path) -> io::Result<()> {
    if let Some(link) = directory_link::inspect(path)? {
        return directory_link::remove(path, link).or_else(|_| fs::remove_file(path));
    }
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        result => result?,
    };
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            remove_entry(&entry?.path())?;
        }
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Symbolic links on Unix, junctions on Windows.
    fn assert_links_to(link: &Path, expected: &Path) {
        let target = directory_link::inspect(link)
            .unwrap()
            .unwrap_or_else(|| panic!("{} is not a link", link.display()))
            .target;
        assert_eq!(
            target.canonicalize().unwrap(),
            expected.canonicalize().unwrap(),
            "{}",
            link.display()
        );
    }

    // Canonical on macOS, where the temp folder sits behind /private; plain
    // on Windows, where canonical paths carry a \\?\ prefix.
    fn real_path(path: &Path) -> PathBuf {
        if cfg!(windows) {
            path.to_path_buf()
        } else {
            path.canonicalize().unwrap()
        }
    }

    fn write_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Fixture\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn classifies_target_folders() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("current");
        let default = temp.path().join("default");
        fs::create_dir_all(&current).unwrap();
        let inspect = |target: &Path| inspect_in(&current, &default, target);

        assert_eq!(
            inspect(&temp.path().join("missing")).unwrap(),
            DataFolder::Empty
        );
        let empty = temp.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        fs::write(empty.join(".DS_Store"), "").unwrap();
        assert_eq!(inspect(&empty).unwrap(), DataFolder::Empty);
        let existing = temp.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("registry.json"), "{}").unwrap();
        assert_eq!(inspect(&existing).unwrap(), DataFolder::Existing);
        let other = temp.path().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("notes.txt"), "").unwrap();
        assert!(inspect(&other).is_err());
        assert!(inspect(&current).is_err());
        assert!(inspect(&current.join("inner")).is_err());
        assert!(inspect(temp.path()).is_err());
        assert!(inspect(Path::new("relative")).is_err());
        // Back to the default folder, which still holds the pointer and CLI.
        fs::create_dir_all(default.join("bin")).unwrap();
        fs::write(default.join(LOCATION_FILE), "{}").unwrap();
        assert_eq!(inspect(&default).unwrap(), DataFolder::Empty);
    }

    #[test]
    fn moves_data_and_relinks_installations() {
        let temp = tempfile::tempdir().unwrap();
        let temp_path = real_path(temp.path());
        let default = temp_path.join("default");
        let target = temp_path.join("moved");
        let external = temp_path.join("external/beta");
        write_skill(&external, "beta");

        let library = SkillLibrary::open_in(&default).unwrap();
        let library_dir = library.config.library_dir.clone();
        write_skill(&library_dir.join("alpha"), "alpha");
        directory_link::create(&external, &library_dir.join("beta")).unwrap();
        fs::write(default.join("tags.json"), "{}").unwrap();
        fs::create_dir_all(default.join("update-cache")).unwrap();
        fs::create_dir_all(default.join("bin")).unwrap();

        let project = temp_path.join("project");
        let installed = project.join(".agents/skills");
        fs::create_dir_all(&installed).unwrap();
        for name in ["alpha", "beta"] {
            directory_link::create(&library_dir.join(name), &installed.join(name)).unwrap();
        }
        directory_link::create(&external, &installed.join("outside")).unwrap();

        let report = move_in(&default, &default, std::slice::from_ref(&project), &target).unwrap();
        assert!(report.stale_links.is_empty());

        assert_eq!(config::read_location(&default), Some(target.clone()));
        assert!(target.join("tags.json").is_file());
        assert!(!target.join("update-cache").exists());
        assert!(!target.join("bin").exists());
        assert!(default.join("bin").is_dir(), "the CLI stays in place");
        assert!(!default.join("skills").exists());
        assert!(!default.join("tags.json").exists());
        assert!(
            external.join("SKILL.md").is_file(),
            "linked sources are untouched"
        );
        // A linked source stays a link.
        assert_links_to(&target.join("skills/beta"), &external);
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(target.join("config.json")).unwrap()).unwrap();
        assert_eq!(
            stored["library_dir"],
            target.join("skills").to_string_lossy().as_ref()
        );
        for name in ["alpha", "beta"] {
            assert_links_to(&installed.join(name), &target.join("skills").join(name));
            assert!(installed.join(name).join("SKILL.md").is_file());
        }
        assert_links_to(&installed.join("outside"), &external);
        let moved = SkillLibrary::open_in(&target).unwrap();
        assert_eq!(moved.config.library_dir, target.join("skills"));
        assert!(
            moved
                .list()
                .unwrap()
                .iter()
                .any(|skill| skill.record.name == "alpha")
        );

        // Moving back to the default folder drops the pointer.
        move_in(&target, &default, &[project], &default).unwrap();
        assert_eq!(config::read_location(&default), None);
        assert!(!target.exists());
        assert_links_to(&installed.join("alpha"), &default.join("skills/alpha"));
    }

    #[test]
    fn copied_data_folders_use_their_own_library() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old");
        let copy = temp.path().join("copy");
        SkillLibrary::open_in(&old).unwrap();
        // Configs written by earlier versions store an absolute library path.
        fs::write(
            old.join("config.json"),
            serde_json::to_vec(&serde_json::json!({"library_dir": old.join("skills")})).unwrap(),
        )
        .unwrap();
        copy_entry(&old, &copy).unwrap();
        assert_eq!(
            AppConfig::load_from(&copy).unwrap().library_dir,
            copy.join("skills")
        );
        fs::remove_dir_all(&old).unwrap();
        assert_eq!(
            AppConfig::load_from(&copy).unwrap().library_dir,
            copy.join("skills")
        );
        // A library chosen outside the data folder is kept.
        let custom = temp.path().join("custom");
        fs::create_dir_all(&custom).unwrap();
        fs::write(
            copy.join("config.json"),
            serde_json::to_vec(&serde_json::json!({"library_dir": custom})).unwrap(),
        )
        .unwrap();
        assert_eq!(AppConfig::load_from(&copy).unwrap().library_dir, custom);
    }
}
