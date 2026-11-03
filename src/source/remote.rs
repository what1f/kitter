//! Read remote refs, trees and link targets without checking out skill files.
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::{SkillLibrary, SkillOrigin, SkillRecord};

const REF_TIMEOUT: Duration = Duration::from_secs(20);
const TREE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RemoteSkill {
    pub repository: String,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub skill_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RemoteKey {
    repository: String,
    reference: String,
}

#[derive(Clone)]
pub(super) struct CheckSkill {
    pub record: SkillRecord,
    source: RemoteSkill,
    files: BTreeMap<String, String>,
    lf_files: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    repository: String,
    reference: String,
    revision: String,
    // Repository file manifest, with targets for symbolic links.
    files: BTreeMap<String, TreeEntry>,
}

#[derive(Serialize, Deserialize)]
struct TreeEntry {
    mode: String,
    oid: String,
    #[serde(default)]
    link_target: Option<String>,
}

impl RemoteSkill {
    /// Normalize repository locators, including skill CLI GitHub shorthand.
    /// An explicit lock-file ref/subpath takes precedence over URL hints.
    pub(super) fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.starts_with('-') || input.is_empty() {
            bail!(t!("errors.source_address_invalid").to_string());
        }
        let mut source = Self {
            repository: input.into(),
            reference: None,
            skill_path: None,
        };
        if let Ok(mut url) = url::Url::parse(input) {
            let parts = url
                .path_segments()
                .map(|parts| parts.collect::<Vec<_>>())
                .unwrap_or_default();
            let tree = match url.host_str() {
                Some("github.com" | "bitbucket.org")
                    if parts.get(2).is_some_and(|p| *p == "tree" || *p == "src") =>
                {
                    Some(2)
                }
                Some("gitlab.com") => parts.windows(2).position(|parts| parts == ["-", "tree"]),
                _ => None,
            };
            if let Some(index) = tree {
                let offset = if parts[index] == "-" { 2 } else { 1 };
                source.reference = parts.get(index + offset).map(|value| (*value).to_owned());
                let path = parts
                    .iter()
                    .skip(index + offset + 1)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("/");
                if !path.is_empty() {
                    source.skill_path = Some(path);
                }
                let repository_path = format!("/{}", parts[..index].join("/"));
                url.set_path(&repository_path);
                url.set_fragment(None);
                url.set_query(None);
                source.repository = url
                    .to_string()
                    .trim_end_matches('/')
                    .trim_end_matches(".git")
                    .into();
            }
        } else if !Path::new(input).is_absolute()
            && !input.starts_with('.')
            && !input.starts_with("git@")
        {
            // owner/repo[@ref] is a GitHub repository; never guess arbitrary URLs.
            let (repo, reference) = input.split_once('@').unwrap_or((input, ""));
            if repo.split('/').count() != 2 || repo.split('/').any(str::is_empty) {
                bail!(t!("errors.update_source_missing").to_string());
            }
            source.repository = format!("https://github.com/{}", repo.trim_end_matches(".git"));
            source.reference = (!reference.is_empty()).then(|| reference.to_owned());
        }
        Ok(source)
    }

    fn key(&self) -> RemoteKey {
        RemoteKey {
            repository: self.repository.clone(),
            reference: self.reference.clone().unwrap_or_else(|| "HEAD".into()),
        }
    }
}

pub(super) fn prepare(
    library: &SkillLibrary,
    record: SkillRecord,
) -> Result<(RemoteKey, CheckSkill)> {
    let source = if let Some(source) = library.remote_skill_source(&record) {
        source.clone()
    } else {
        match &record.origin {
            SkillOrigin::Git { repository, subdir } => {
                let mut source = RemoteSkill::parse(repository)?;
                if subdir.is_some() {
                    source.skill_path = subdir.clone();
                }
                source
            }
            SkillOrigin::Npx {
                repository, skill, ..
            } => super::legacy_npx_source(library, repository, skill)
                .map(Ok)
                .unwrap_or_else(|| RemoteSkill::parse(repository))?,
            _ => bail!(t!("errors.update_source_missing").to_string()),
        }
    };
    let root = library.skill_path_by_storage(&record.storage_name)?;
    let original = library.trigger_source_by_storage(&record.storage_name);
    let mut files = BTreeMap::new();
    let mut lf_files = BTreeMap::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
    {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(&root)?
            .to_str()
            .context(t!("errors.update_source_missing").to_string())?
            .replace('\\', "/");
        let bytes = if path == "SKILL.md" && original.is_some() {
            original.unwrap().skill_md.as_bytes().to_vec()
        } else if path == "agents/openai.yaml" && original.is_some() {
            match &original.unwrap().openai_yaml {
                Some(content) => content.as_bytes().to_vec(),
                None => continue,
            }
        } else if entry.file_type().is_symlink() {
            fs::read_link(entry.path())?
                .to_str()
                .context(t!("errors.update_source_missing").to_string())?
                .as_bytes()
                .to_vec()
        } else {
            fs::read(entry.path())?
        };
        if !entry.file_type().is_symlink()
            && let Some(normalized) = normalize_crlf(&bytes)
        {
            lf_files.insert(path.clone(), blob_hash(&normalized));
        }
        files.insert(path, blob_hash(&bytes));
    }
    Ok((
        source.key(),
        CheckSkill {
            record,
            source,
            files,
            lf_files,
        },
    ))
}

fn blob_hash(bytes: &[u8]) -> String {
    let mut hash = sha1_smol::Sha1::new();
    hash.update(format!("blob {}\0", bytes.len()).as_bytes());
    hash.update(bytes);
    hash.digest().to_string()
}

// Git stores LF for text files that a checkout can materialize as CRLF. Keep
// the exact hash too, and never treat binary data or lone CRs as text conversion.
fn normalize_crlf(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.contains(&0)
        || !bytes.windows(2).any(|pair| pair == b"\r\n")
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'))
    {
        return None;
    }
    Some(
        bytes
            .iter()
            .copied()
            .filter(|byte| *byte != b'\r')
            .collect(),
    )
}

pub(super) fn check_group(
    data_dir: &Path,
    key: &RemoteKey,
    skills: &[CheckSkill],
) -> Result<Vec<Result<bool>>> {
    let snapshot = snapshot(data_dir, key)?;
    Ok(skills
        .iter()
        .map(|skill| compare(&snapshot, skill))
        .collect())
}

fn skill_path(snapshot: &Snapshot, skill: &CheckSkill) -> Result<String> {
    if let Some(path) = &skill.source.skill_path {
        let path = path.trim_matches('/').trim_end_matches("/SKILL.md");
        if !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            && !path.is_empty()
        {
            bail!(t!("errors.update_source_missing").to_string());
        }
        return Ok(path.into());
    }
    if snapshot.files.contains_key("SKILL.md") {
        return Ok(String::new());
    }
    let name = match &skill.record.origin {
        SkillOrigin::Npx { skill, .. } => skill,
        _ => &skill.record.name,
    };
    let paths = snapshot
        .files
        .keys()
        .filter_map(|path| path.strip_suffix("/SKILL.md"))
        .filter(|path| path.rsplit('/').next() == Some(name))
        .collect::<Vec<_>>();
    match paths.as_slice() {
        [path] => Ok((*path).into()),
        _ => bail!(t!("errors.update_source_missing").to_string()),
    }
}

fn compare(snapshot: &Snapshot, skill: &CheckSkill) -> Result<bool> {
    let path = skill_path(snapshot, skill)?;
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    let mut remote = BTreeMap::new();
    for path in snapshot.files.keys() {
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        add_comparison_entry(snapshot, path, relative, &skill.files, &mut remote, 0)?;
    }
    if !remote.contains_key("SKILL.md") {
        // A removed skill cannot be upgraded using the current source.
        bail!(t!("errors.repository_skill_missing").to_string());
    }
    Ok(remote.len() != skill.files.len()
        || remote.iter().any(|(path, oid)| {
            skill.files.get(path) != Some(oid) && skill.lf_files.get(path) != Some(oid)
        }))
}

// Skill installers can materialize symbolic links as ordinary files or folders.
// Compare their content too, while preserving exact Git-link comparisons when
// the installed copy still has the original link.
fn add_comparison_entry(
    snapshot: &Snapshot,
    source: &str,
    destination: &str,
    local: &BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
    depth: usize,
) -> Result<()> {
    let entry = &snapshot.files[source];
    // A submodule needs its own revision/source, including through a link.
    if entry.mode == "160000" {
        bail!(t!("errors.update_source_missing").to_string());
    }
    if entry.mode != "120000" || local.get(destination) == Some(&entry.oid) {
        files.insert(destination.into(), entry.oid.clone());
        return Ok(());
    }
    if depth >= 16 {
        bail!(t!("errors.update_source_missing").to_string());
    }
    let target = entry
        .link_target
        .as_deref()
        .context(t!("errors.update_source_missing").to_string())?;
    let mut resolved = Path::new(source)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for component in Path::new(target).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir if resolved.pop() => {}
            _ => bail!(t!("errors.update_source_missing").to_string()),
        }
    }
    let resolved = resolved.to_string_lossy().replace('\\', "/");
    if snapshot.files.contains_key(&resolved) {
        return add_comparison_entry(snapshot, &resolved, destination, local, files, depth + 1);
    }
    let prefix = if resolved.is_empty() {
        String::new()
    } else {
        format!("{resolved}/")
    };
    let mut found = false;
    for path in snapshot.files.keys() {
        if let Some(relative) = path.strip_prefix(&prefix) {
            found = true;
            add_comparison_entry(
                snapshot,
                path,
                &format!("{destination}/{relative}"),
                local,
                files,
                depth + 1,
            )?;
        }
    }
    if !found {
        bail!(t!("errors.update_source_missing").to_string());
    }
    Ok(())
}

fn resolve_revision(key: &RemoteKey) -> Result<String> {
    // A commit pin is immutable. Branches/tags still need a fresh refs query.
    if is_oid(&key.reference) {
        return Ok(key.reference.clone());
    }
    let mut refs = super::tool_command("git");
    refs.args(["ls-remote", "--exit-code", "--", &key.repository]);
    if key.reference == "HEAD" {
        refs.arg("HEAD");
    } else if key.reference.starts_with("refs/") {
        refs.args([key.reference.clone(), format!("{}^{{}}", key.reference)]);
    } else {
        refs.args([
            format!("refs/heads/{}", key.reference),
            format!("refs/tags/{}", key.reference),
            format!("refs/tags/{}^{{}}", key.reference),
        ]);
    }
    let output = String::from_utf8(git_output(&mut refs, REF_TIMEOUT)?)?;
    let revisions = output
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect::<Vec<_>>();
    let revision = revisions
        .iter()
        .find(|(_, name)| *name == format!("refs/heads/{}", key.reference))
        .or_else(|| revisions.iter().find(|(_, name)| name.ends_with("^{}")))
        .or_else(|| revisions.first())
        .map(|(oid, _)| (*oid).to_owned())
        .context(t!("errors.update_source_missing").to_string())?;
    if !is_oid(&revision) {
        bail!(t!("errors.update_source_missing").to_string());
    }
    Ok(revision)
}

pub(super) fn is_oid(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn snapshot(data_dir: &Path, key: &RemoteKey) -> Result<Snapshot> {
    let revision = resolve_revision(key)?;
    let mut hash = sha1_smol::Sha1::new();
    hash.update(key.repository.as_bytes());
    hash.update(b"\0");
    hash.update(key.reference.as_bytes());
    let cache_dir = data_dir.join("update-cache");
    let cache_path = cache_dir.join(format!("{}.json", hash.digest()));
    if let Ok(bytes) = fs::read(&cache_path)
        && let Ok(cached) = serde_json::from_slice::<Snapshot>(&bytes)
        && cached.repository == key.repository
        && cached.reference == key.reference
        && cached.revision == revision
        && cached
            .files
            .values()
            .all(|entry| entry.mode != "120000" || entry.link_target.is_some())
    {
        return Ok(cached);
    }
    let temp = tempfile::tempdir()?;
    let mut init = super::tool_command("git");
    init.args(["init", "--bare"]).arg(temp.path());
    git_output(&mut init, REF_TIMEOUT)?;
    let mut origin = super::tool_command("git");
    origin
        .arg("-C")
        .arg(temp.path())
        .args(["remote", "add", "origin", &key.repository]);
    git_output(&mut origin, REF_TIMEOUT)?;
    let mut fetch = super::tool_command("git");
    fetch.arg("-C").arg(temp.path()).args([
        "-c",
        "fetch.recurseSubmodules=false",
        "fetch",
        "--depth=1",
        "--filter=blob:none",
        "--no-tags",
        "--no-recurse-submodules",
        "--",
        "origin",
        &revision,
    ]);
    git_output(&mut fetch, TREE_TIMEOUT)?;
    let mut tree = super::tool_command("git");
    tree.arg("-C")
        .arg(temp.path())
        .args(["ls-tree", "-rz", "--full-tree", &revision]);
    let output = git_output(&mut tree, REF_TIMEOUT)?;
    let mut files = BTreeMap::new();
    for entry in output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry)?;
        let (metadata, path) = entry
            .split_once('\t')
            .context(t!("errors.update_source_missing").to_string())?;
        let parts = metadata.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3 {
            bail!(t!("errors.update_source_missing").to_string());
        }
        let link_target = if parts[0] == "120000" {
            let mut show = super::tool_command("git");
            show.arg("-C")
                .arg(temp.path())
                .args(["show", &format!("{revision}:{path}")]);
            Some(String::from_utf8(git_output(&mut show, TREE_TIMEOUT)?)?)
        } else {
            None
        };
        files.insert(
            path.into(),
            TreeEntry {
                mode: parts[0].into(),
                oid: parts[2].into(),
                link_target,
            },
        );
    }
    let snapshot = Snapshot {
        repository: key.repository.clone(),
        reference: key.reference.clone(),
        revision,
        files,
    };
    // Cache failures only lose an optimization. Unique temporary names keep
    // simultaneous CLI and desktop checks from corrupting each other's cache.
    if fs::create_dir_all(&cache_dir).is_ok()
        && let Ok(mut cache) = tempfile::NamedTempFile::new_in(&cache_dir)
        && serde_json::to_writer(cache.as_file_mut(), &snapshot).is_ok()
    {
        let _ = cache.persist(&cache_path);
    }
    Ok(snapshot)
}

/// Capture to files so a full stderr pipe cannot deadlock the timeout.
pub(super) fn git_output(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    let stdout = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .context(crate::user_error!("errors.tool_unavailable"))?;
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        waited => {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = super::tool_command("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            waited?;
            bail!(t!("errors.update_check_timeout").to_string());
        }
    };
    use std::io::{Read, Seek};
    let mut stdout = stdout;
    stdout.rewind()?;
    let mut output = Vec::new();
    stdout.read_to_end(&mut output)?;
    if !status.success() {
        let mut stderr = stderr;
        stderr.rewind()?;
        let mut message = String::new();
        stderr.read_to_string(&mut message)?;
        bail!(crate::user_error!("errors.fetch_failed").detail(message.trim().to_owned()));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().into()
    }

    fn commit(root: &Path, message: &str) {
        git(root, &["add", "."]);
        git(
            root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    fn fixture() -> (tempfile::TempDir, SkillLibrary) {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "--object-format=sha1", "-b", "main"]);
        for name in ["alpha", "beta"] {
            fs::create_dir_all(repo.join("skills").join(name)).unwrap();
            fs::write(
                repo.join("skills").join(name).join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Fixture\n---\nOld content\n"),
            )
            .unwrap();
        }
        commit(&repo, "initial");
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        for name in ["alpha", "beta"] {
            let record = SkillRecord {
                name: name.into(),
                storage_name: String::new(),
                description: String::new(),
                custom_description: None,
                origin: SkillOrigin::Git {
                    repository: repo.to_str().unwrap().into(),
                    subdir: Some(format!("skills/{name}")),
                },
                update_available: false,
                group_id: None,
                last_operated_at: 0,
            };
            library
                .import(&repo.join("skills").join(name), record)
                .unwrap();
        }
        (temp, library)
    }

    #[test]
    fn checks_only_skill_files_and_reuses_cached_manifest() {
        let (temp, mut library) = fixture();
        let repo = temp.path().join("repo");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        let cache = fs::read_dir(library.data_dir().join("update-cache"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let before = fs::metadata(&cache).unwrap().modified().unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        assert_eq!(
            fs::metadata(&cache).unwrap().modified().unwrap(),
            before,
            "unchanged refs reuse cached tree data"
        );
        fs::write(repo.join("README.md"), "unrelated repository change").unwrap();
        commit(&repo, "readme");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        fs::write(repo.join("skills/alpha/reference.txt"), "new reference").unwrap();
        commit(&repo, "alpha reference");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 1);
        assert!(library.record("alpha").unwrap().update_available);
        assert!(!library.record("beta").unwrap().update_available);
        assert!(
            !library
                .skill_path("alpha")
                .unwrap()
                .join("reference.txt")
                .exists(),
            "checking never installs content"
        );
        let record = library.record("alpha").unwrap();
        library
            .replace_by_storage(
                &repo.join("skills/alpha"),
                record.storage_name.clone(),
                record,
            )
            .unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
    }

    #[test]
    fn crlf_checkouts_are_current_but_content_and_binary_changes_are_detected() {
        let (temp, mut library) = fixture();
        let repo = temp.path().join("repo");
        let checkout = temp.path().join("crlf-checkout");
        git(
            temp.path(),
            &[
                "-c",
                "core.autocrlf=true",
                "clone",
                "-q",
                repo.to_str().unwrap(),
                checkout.to_str().unwrap(),
            ],
        );
        let crlf = fs::read(checkout.join("skills/alpha/SKILL.md")).unwrap();
        assert!(crlf.windows(2).any(|pair| pair == b"\r\n"));
        let record = library.record("alpha").unwrap();
        library
            .replace_by_storage(
                &checkout.join("skills/alpha"),
                record.storage_name.clone(),
                record,
            )
            .unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        // The cache must preserve the same result on later checks.
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        fs::write(repo.join("skills/alpha/reference.txt"), "new content\n").unwrap();
        commit(&repo, "real update");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 1);
        let binary = b"binary\0payload\r\n";
        assert!(normalize_crlf(binary).is_none());
        assert!(normalize_crlf(b"lone\rcarriage\r\n").is_none());
        fs::write(repo.join("skills/beta/data.bin"), binary).unwrap();
        commit(&repo, "binary reference");
        let local = library.skill_path("beta").unwrap();
        fs::write(local.join("data.bin"), b"binary\0payload\n").unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 2);
        fs::write(local.join("data.bin"), binary).unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 1);
    }

    #[test]
    fn changed_trigger_settings_do_not_look_like_upstream_updates() {
        let (_temp, mut library) = fixture();
        let storage = library.record("alpha").unwrap().storage_name;
        library
            .set_trigger_mode_by_storage(&storage, crate::TriggerMode::Manual)
            .unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn materialized_links_compare_content_and_detect_later_changes() {
        use std::os::unix::fs::symlink;
        let (temp, mut library) = fixture();
        let repo = temp.path().join("repo");
        symlink("../beta", repo.join("skills/alpha/shared")).unwrap();
        symlink("../beta/SKILL.md", repo.join("skills/alpha/linked.md")).unwrap();
        commit(&repo, "shared references");
        let local = library.skill_path("alpha").unwrap();
        crate::library::copy_tree(&repo.join("skills/beta"), &local.join("shared")).unwrap();
        fs::copy(repo.join("skills/beta/SKILL.md"), local.join("linked.md")).unwrap();
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        // A warm cached check must retain link-target information.
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);
        fs::write(repo.join("skills/beta/new.txt"), "new reference").unwrap();
        commit(&repo, "changed shared references");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 2);
        assert!(library.record("alpha").unwrap().update_available);
    }

    #[test]
    fn failed_remote_preserves_previous_update_status() {
        let (temp, mut library) = fixture();
        library.set_update_available("alpha", true).unwrap();
        fs::rename(temp.path().join("repo"), temp.path().join("offline")).unwrap();
        assert!(super::super::check_updates(&mut library).is_err());
        assert!(library.record("alpha").unwrap().update_available);
    }

    #[test]
    fn lock_locator_supports_custom_skill_names_and_pinned_refs() {
        let (temp, mut library) = fixture();
        let repo = temp.path().join("repo");
        git(&repo, &["branch", "pinned"]);
        let origin = SkillOrigin::Npx {
            repository: repo.to_str().unwrap().into(),
            skill: "display-name".into(),
            source_hash: None,
        };
        library
            .import(
                &repo.join("skills/alpha"),
                SkillRecord {
                    name: "display-name".into(),
                    storage_name: String::new(),
                    description: String::new(),
                    custom_description: None,
                    origin: origin.clone(),
                    update_available: false,
                    group_id: None,
                    last_operated_at: 0,
                },
            )
            .unwrap();
        library
            .set_remote_skill_source(
                origin.identity_key("display-name"),
                RemoteSkill {
                    repository: repo.to_str().unwrap().into(),
                    reference: Some("pinned".into()),
                    skill_path: Some("skills/alpha".into()),
                },
            )
            .unwrap();
        fs::write(repo.join("skills/alpha/new.txt"), "main branch only").unwrap();
        commit(&repo, "main changed");
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 1);
        assert!(!library.record("display-name").unwrap().update_available);
        assert!(
            !library.data_dir().join("npx-sources").exists(),
            "npx checks do not create or update an install workspace"
        );
    }

    #[test]
    fn ambiguous_legacy_skill_paths_are_not_guessed() {
        let (temp, library) = fixture();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("duplicate/alpha")).unwrap();
        fs::copy(
            repo.join("skills/alpha/SKILL.md"),
            repo.join("duplicate/alpha/SKILL.md"),
        )
        .unwrap();
        commit(&repo, "ambiguous skill");
        let mut record = library.record("alpha").unwrap();
        record.origin = SkillOrigin::Npx {
            repository: repo.to_str().unwrap().into(),
            skill: "alpha".into(),
            source_hash: None,
        };
        let (key, skill) = prepare(&library, record).unwrap();
        assert!(
            check_group(library.data_dir(), &key, &[skill])
                .unwrap()
                .remove(0)
                .is_err()
        );
    }

    #[test]
    fn parses_repository_branch_and_skill_path() {
        let source =
            RemoteSkill::parse("https://github.com/owner/repo/tree/main/skills/alpha").unwrap();
        assert_eq!(source.repository, "https://github.com/owner/repo");
        assert_eq!(source.reference.as_deref(), Some("main"));
        assert_eq!(source.skill_path.as_deref(), Some("skills/alpha"));
        assert_eq!(
            RemoteSkill::parse("owner/repo@stable")
                .unwrap()
                .reference
                .as_deref(),
            Some("stable")
        );
        assert!(RemoteSkill::parse("--upload-pack=bad").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn commands_time_out_without_waiting_for_a_full_pipe() {
        let mut command = Command::new("sh");
        command.args(["-c", "while :; do printf 'output output output\\n'; done"]);
        let started = std::time::Instant::now();
        assert!(git_output(&mut command, Duration::from_millis(40)).is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
