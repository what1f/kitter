use std::{collections::HashSet, fs, path::PathBuf};

use ignore::WalkBuilder;

use super::SkillRoot;

#[derive(Clone, Copy)]
pub(super) enum ScanProfile {
    DirectChildren,
    Recursive {
        max_depth: usize,
        max_directories: usize,
        max_entries: usize,
    },
    PiIgnored,
}

pub(super) fn scan(root: &SkillRoot, profile: ScanProfile) -> Vec<PathBuf> {
    if let Some(path) = &root.exact_skill_file {
        return path
            .is_file()
            .then(|| vec![path.clone()])
            .unwrap_or_default();
    }
    if !root.path.is_dir() {
        return Vec::new();
    }
    if root.flat_markdown_only {
        return flat_markdown(root);
    }
    if root.direct_children_only {
        return direct_children(root);
    }
    let mut files = match profile {
        ScanProfile::DirectChildren => direct_children(root),
        ScanProfile::Recursive {
            max_depth,
            max_directories,
            max_entries,
        } => recursive(root, max_depth, max_directories, max_entries),
        ScanProfile::PiIgnored => pi_ignored(root),
    };
    files.sort();
    files.dedup();
    files
}

fn flat_markdown(root: &SkillRoot) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(&root.path) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "md")
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn direct_children(root: &SkillRoot) -> Vec<PathBuf> {
    let mut result = root
        .path
        .join("SKILL.md")
        .is_file()
        .then(|| vec![root.path.join("SKILL.md")])
        .unwrap_or_default();
    let Ok(entries) = fs::read_dir(&root.path) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest = path.join("SKILL.md");
            if manifest.is_file() {
                result.push(manifest);
            }
        } else if root.include_root_markdown
            && path.extension().is_some_and(|extension| extension == "md")
        {
            result.push(path);
        }
    }
    result
}

fn recursive(
    root: &SkillRoot,
    max_depth: usize,
    max_directories: usize,
    max_entries: usize,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![(root.path.clone(), 0usize)];
    let mut visited = HashSet::new();
    let mut directories = 0usize;
    let mut entries_seen = 0usize;
    while let Some((directory, depth)) = stack.pop() {
        if directories >= max_directories || entries_seen >= max_entries {
            break;
        }
        let canonical = fs::canonicalize(&directory).unwrap_or_else(|_| directory.clone());
        if !visited.insert(canonical) {
            continue;
        }
        directories += 1;
        let manifest = directory.join("SKILL.md");
        if manifest.is_file() {
            result.push(manifest);
            continue;
        }
        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries.into_iter().rev() {
            entries_seen += 1;
            if entries_seen > max_entries {
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let path = entry.path();
            if directory == root.path
                && root.include_root_markdown
                && path.is_file()
                && path.extension().is_some_and(|extension| extension == "md")
            {
                result.push(path);
            } else if path.is_dir()
                && (root.follow_directory_symlinks
                    || !entry.file_type().is_ok_and(|kind| kind.is_symlink()))
            {
                stack.push((path, depth + 1));
            }
        }
    }
    result
}

fn pi_ignored(root: &SkillRoot) -> Vec<PathBuf> {
    let root_manifest = root.path.join("SKILL.md");
    if root_manifest.is_file() {
        return vec![root_manifest];
    }
    let mut result = Vec::new();
    let mut builder = WalkBuilder::new(&root.path);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .ignore(true)
        .parents(true)
        .follow_links(true);
    for entry in builder.build().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "SKILL.md")
            || (root.include_root_markdown
                && path.parent() == Some(root.path.as_path())
                && path.extension().is_some_and(|extension| extension == "md"))
        {
            result.push(path.to_path_buf());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::effective_skills::{SkillRoot, SkillScope};

    #[test]
    fn direct_children_do_not_recurse() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("direct")).unwrap();
        fs::create_dir_all(temp.path().join("group/nested")).unwrap();
        fs::write(temp.path().join("direct/SKILL.md"), "---\n---").unwrap();
        fs::write(temp.path().join("group/nested/SKILL.md"), "---\n---").unwrap();
        let root = SkillRoot::new(temp.path().to_path_buf(), SkillScope::User);
        let files = scan(&root, ScanProfile::DirectChildren);
        assert_eq!(files, vec![temp.path().join("direct/SKILL.md")]);
    }

    #[test]
    fn pi_scan_honors_ignore_files() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("kept")).unwrap();
        fs::create_dir_all(temp.path().join("ignored")).unwrap();
        fs::write(temp.path().join(".ignore"), "ignored/\n").unwrap();
        fs::write(temp.path().join("kept/SKILL.md"), "---\n---").unwrap();
        fs::write(temp.path().join("ignored/SKILL.md"), "---\n---").unwrap();
        let root = SkillRoot::new(temp.path().to_path_buf(), SkillScope::User);
        let files = scan(&root, ScanProfile::PiIgnored);
        assert_eq!(files, vec![temp.path().join("kept/SKILL.md")]);
    }
}
