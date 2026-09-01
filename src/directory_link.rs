use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryLinkKind {
    SymbolicLink,
    #[cfg(windows)]
    Junction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectoryLink {
    pub kind: DirectoryLinkKind,
    pub target: PathBuf,
}

pub(crate) fn inspect(path: &Path) -> io::Result<Option<DirectoryLink>> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    #[cfg(windows)]
    if let Ok(target) = junction::get_target(path) {
        return Ok(Some(DirectoryLink {
            kind: DirectoryLinkKind::Junction,
            target,
        }));
    }

    if metadata.file_type().is_symlink() {
        let target = resolve_target(path, fs::read_link(path)?);
        return Ok(Some(DirectoryLink {
            kind: DirectoryLinkKind::SymbolicLink,
            target,
        }));
    }

    Ok(None)
}

pub(crate) fn create(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    {
        match junction::create(target, link) {
            Ok(()) => Ok(()),
            Err(error) => {
                // junction::create creates the directory before attaching the reparse
                // point. Do not leave an empty path behind when that second step fails.
                if junction::get_target(link).is_err() {
                    let _ = fs::remove_dir(link);
                }
                Err(error)
            }
        }
    }
}

pub(crate) fn remove(path: &Path, link: DirectoryLink) -> io::Result<()> {
    match link.kind {
        DirectoryLinkKind::SymbolicLink => remove_symbolic_link(path),
        #[cfg(windows)]
        DirectoryLinkKind::Junction => {
            junction::delete(path)?;
            fs::remove_dir(path)
        }
    }
}

fn resolve_target(link: &Path, target: PathBuf) -> PathBuf {
    if target.is_absolute() {
        target
    } else {
        link.parent().unwrap_or_else(|| Path::new("")).join(target)
    }
}

#[cfg(unix)]
fn remove_symbolic_link(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_symbolic_link(path: &Path) -> io::Result<()> {
    fs::remove_dir(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_inspects_and_removes_a_directory_link_without_touching_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let link = temp.path().join("link");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("canary"), "keep").unwrap();

        create(&target, &link).unwrap();
        let inspected = inspect(&link).unwrap().unwrap();
        assert_eq!(
            inspected.target.canonicalize().unwrap(),
            target.canonicalize().unwrap()
        );

        remove(&link, inspected).unwrap();
        assert!(link.symlink_metadata().is_err());
        assert_eq!(fs::read_to_string(target.join("canary")).unwrap(), "keep");
    }

    #[test]
    fn ordinary_directory_is_not_a_directory_link() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("directory");
        fs::create_dir(&directory).unwrap();

        assert_eq!(inspect(&directory).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_relative_symbolic_link_targets_from_the_link_parent() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("parent");
        let target = temp.path().join("target");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&target).unwrap();
        let link = parent.join("link");
        symlink("../target", &link).unwrap();

        assert_eq!(
            inspect(&link).unwrap().unwrap().target,
            parent.join("../target")
        );
    }
}
