//! Read skills.sh / GitHub sources with plain git when npx is unavailable.
//! Discovery mirrors the skills CLI so both paths offer the same skills.
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};

use super::{
    ScannedSkill,
    remote::{RemoteSkill, git_output, is_oid},
};

const CLONE_TIMEOUT: Duration = Duration::from_secs(300);
const SKIP_DIRS: [&str; 5] = ["node_modules", ".git", "dist", "build", "__pycache__"];
const CONTAINER_DEPTH: usize = 3;
const FALLBACK_DEPTH: usize = 5;
const AGENT_SKILL_DIRS: [&str; 27] = [
    ".agents/skills",
    ".claude/skills",
    ".cline/skills",
    ".codebuddy/skills",
    ".codex/skills",
    ".commandcode/skills",
    ".continue/skills",
    ".github/skills",
    ".goose/skills",
    ".grok/skills",
    ".iflow/skills",
    ".junie/skills",
    ".kilocode/skills",
    ".kimchi/skills",
    ".kiro/skills",
    ".minimax/skills",
    ".mux/skills",
    ".neovate/skills",
    ".opencode/skills",
    ".openhands/skills",
    ".pi/skills",
    ".qoder/skills",
    ".roo/skills",
    ".trae/skills",
    ".windsurf/skills",
    ".zcode/skills",
    ".zencoder/skills",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fetcher {
    Npx,
    Git,
}

impl Fetcher {
    /// Prefer the skills CLI; fall back to git, which update checks need anyway.
    pub(super) fn detect() -> Result<Self> {
        if super::find_tool("npx").is_some() {
            Ok(Self::Npx)
        } else if super::find_tool("git").is_some() {
            Ok(Self::Git)
        } else {
            crate::user_bail!("errors.source_tools_missing")
        }
    }
}

/// A repository plus an optional subpath (`skill_path`) and skill name filter.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub source: RemoteSkill,
    pub skill: Option<String>,
}

/// Accept the source forms of `npx skills add`: repository URLs (including
/// GitHub/GitLab tree URLs), `owner/repo[/path|@skill]`, `github:` shorthand,
/// `#ref[@skill]` fragments, and skills.sh skill pages.
pub(super) fn parse_target(input: &str) -> Result<Target> {
    let input = input.trim();
    let (base, fragment) = match input.split_once('#') {
        Some((base, fragment)) if !fragment.is_empty() => (base, Some(fragment)),
        _ => (input, None),
    };
    let (fragment_ref, fragment_skill) = match fragment {
        Some(fragment) => match fragment.split_once('@') {
            Some((reference, skill)) => (non_empty(reference), non_empty(skill)),
            None => (non_empty(fragment), None),
        },
        None => (None, None),
    };

    let mut target = if let Some(path) = skills_sh_path(base) {
        let mut parts = path.split('/').filter(|part| !part.is_empty());
        let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else {
            crate::user_bail!("errors.source_address_invalid");
        };
        Target {
            source: github(owner, repo, None),
            skill: parts.next().map(str::to_owned),
        }
    } else if let Some(shorthand) = shorthand(base) {
        parse_shorthand(shorthand)?
    } else {
        Target {
            source: RemoteSkill::parse(base)?,
            skill: None,
        }
    };
    if fragment_ref.is_some() {
        target.source.reference = fragment_ref;
    }
    if fragment_skill.is_some() {
        target.skill = fragment_skill;
    }
    Ok(target)
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn skills_sh_path(input: &str) -> Option<&str> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let path = rest.strip_prefix("skills.sh/")?;
    Some(path.split(['?', '#']).next().unwrap_or(path))
}

fn shorthand(input: &str) -> Option<&str> {
    if let Some(rest) = input.strip_prefix("github:") {
        return Some(rest);
    }
    (!input.contains(':')
        && !input.starts_with('.')
        && !input.starts_with('/')
        && !input.starts_with('\\')
        && input.contains('/'))
    .then_some(input)
}

fn parse_shorthand(input: &str) -> Result<Target> {
    let (owner, rest) = input
        .split_once('/')
        .context(crate::user_error!("errors.source_address_invalid"))?;
    if let Some((repo, skill)) = rest.split_once('@')
        && !repo.contains('/')
    {
        return Ok(Target {
            source: github(owner, repo, None),
            skill: non_empty(skill),
        });
    }
    let (repo, path) = rest.split_once('/').unwrap_or((rest, ""));
    if owner.is_empty() || repo.is_empty() {
        crate::user_bail!("errors.source_address_invalid");
    }
    let path = path.trim_matches('/');
    Ok(Target {
        source: github(owner, repo, non_empty(path)),
        skill: None,
    })
}

fn github(owner: &str, repo: &str, skill_path: Option<String>) -> RemoteSkill {
    RemoteSkill {
        repository: format!(
            "https://github.com/{owner}/{}",
            repo.trim_end_matches(".git")
        ),
        reference: None,
        skill_path,
    }
}

/// Shallow checkout of the source's ref (default branch when unset).
pub(super) fn checkout(source: &RemoteSkill, destination: &Path) -> Result<()> {
    let git = || {
        let mut command = super::tool_command("git");
        command.args([
            "-c",
            "core.longpaths=true",
            "-c",
            "advice.detachedHead=false",
        ]);
        command
    };
    match source.reference.as_deref() {
        // A commit cannot be cloned by name; fetch exactly that object.
        Some(oid) if is_oid(oid) => {
            fs::create_dir_all(destination)?;
            git_output(git().arg("-C").arg(destination).arg("init"), CLONE_TIMEOUT)?;
            git_output(
                git().arg("-C").arg(destination).args([
                    "remote",
                    "add",
                    "origin",
                    &source.repository,
                ]),
                CLONE_TIMEOUT,
            )?;
            git_output(
                git().arg("-C").arg(destination).args([
                    "fetch",
                    "--depth=1",
                    "--no-tags",
                    "--",
                    "origin",
                    oid,
                ]),
                CLONE_TIMEOUT,
            )?;
            git_output(
                git()
                    .arg("-C")
                    .arg(destination)
                    .args(["checkout", "--detach", "FETCH_HEAD"]),
                CLONE_TIMEOUT,
            )?;
        }
        reference => {
            let mut clone = git();
            clone.args(["clone", "--depth=1", "--no-tags"]);
            if let Some(reference) = reference {
                clone.args(["--branch", reference]);
            }
            clone.args(["--", &source.repository]).arg(destination);
            git_output(&mut clone, CLONE_TIMEOUT)?;
        }
    }
    Ok(())
}

/// Skills in a checkout, keyed by the source each one updates from.
pub(super) fn discover(
    checkout: &Path,
    source: &RemoteSkill,
    skill: Option<&str>,
) -> Result<Vec<(ScannedSkill, RemoteSkill)>> {
    let subpath = source.skill_path.as_deref().unwrap_or("").trim_matches('/');
    if !Path::new(subpath)
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        crate::user_bail!("errors.source_address_invalid");
    }
    let search = checkout.join(subpath);
    let mut found = Discovery::default();
    if !found.add(&search)? {
        let mut deep = vec![search.join("skills")];
        deep.extend(
            ["skills/.curated", "skills/.experimental", "skills/.system"]
                .iter()
                .chain(AGENT_SKILL_DIRS.iter())
                .map(|dir| search.join(dir)),
        );
        found.walk(&search, 1, 1)?;
        for dir in &deep {
            found.walk(dir, CONTAINER_DEPTH, 1)?;
        }
        for dir in plugin_skill_dirs(&search) {
            found.walk(&dir, 1, 1)?;
        }
        if found.skills.is_empty() {
            found.walk_all(&search, 0)?;
        }
    }
    let mut skills = Vec::new();
    for scanned in found.skills {
        if skill.is_some_and(|skill| !scanned.name.eq_ignore_ascii_case(skill)) {
            continue;
        }
        let relative = scanned
            .path
            .strip_prefix(checkout)?
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let remote = RemoteSkill {
            repository: source.repository.clone(),
            reference: source.reference.clone(),
            skill_path: non_empty(&relative),
        };
        skills.push((scanned, remote));
    }
    Ok(skills)
}

#[derive(Default)]
struct Discovery {
    skills: Vec<ScannedSkill>,
    names: HashSet<String>,
    paths: HashSet<PathBuf>,
}

impl Discovery {
    /// Whether `dir` is a skill. Later duplicates of a name are ignored.
    fn add(&mut self, dir: &Path) -> Result<bool> {
        let marker = dir.join("SKILL.md");
        if !marker.is_file() {
            return Ok(false);
        }
        if !self.paths.insert(dir.to_path_buf()) {
            return Ok(true);
        }
        let (front_name, description) = crate::library::read_frontmatter(&marker)?;
        let name = front_name
            .or_else(|| {
                dir.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .context(crate::user_error!("errors.skill_name_unreadable"))?;
        if self.names.insert(name.clone()) {
            self.skills.push(ScannedSkill {
                name,
                description: description.unwrap_or_default(),
                path: dir.to_path_buf(),
            });
        }
        Ok(true)
    }

    /// Stop below a discovered skill; never descend through ignored folders.
    fn walk(&mut self, dir: &Path, max_depth: usize, depth: usize) -> Result<()> {
        for child in child_dirs(dir) {
            if self.add(&child)?
                || depth >= max_depth
                || child
                    .file_name()
                    .is_some_and(|name| SKIP_DIRS.iter().any(|skip| name == *skip))
            {
                continue;
            }
            self.walk(&child, max_depth, depth + 1)?;
        }
        Ok(())
    }

    fn walk_all(&mut self, dir: &Path, depth: usize) -> Result<()> {
        if depth > FALLBACK_DEPTH {
            return Ok(());
        }
        self.add(dir)?;
        for child in child_dirs(dir) {
            if !child
                .file_name()
                .is_some_and(|name| SKIP_DIRS.iter().any(|skip| name == *skip))
            {
                self.walk_all(&child, depth + 1)?;
            }
        }
        Ok(())
    }
}

fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

/// Skill folders declared by Claude plugin manifests (local `./` paths only).
fn plugin_skill_dirs(root: &Path) -> Vec<PathBuf> {
    fn read(path: &Path) -> Option<serde_json::Value> {
        serde_json::from_slice(&fs::read(path).ok()?).ok()
    }
    fn relative(value: &str) -> Option<&Path> {
        let path = Path::new(value.strip_prefix("./")?);
        path.components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
            .then_some(path)
    }
    fn add(dirs: &mut Vec<PathBuf>, base: PathBuf, skills: Option<&serde_json::Value>) {
        for skill in skills
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter_map(relative)
        {
            if let Some(parent) = base.join(skill).parent() {
                dirs.push(parent.to_path_buf());
            }
        }
        dirs.push(base.join("skills"));
    }

    let mut dirs = Vec::new();
    if let Some(manifest) = read(&root.join(".claude-plugin/marketplace.json")) {
        let plugin_root = match manifest.pointer("/metadata/pluginRoot") {
            None => Some(Path::new("")),
            Some(value) => value.as_str().and_then(relative),
        };
        if let Some(plugin_root) = plugin_root {
            for plugin in manifest
                .get("plugins")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                let source = match plugin.get("source") {
                    None => Some(Path::new("")),
                    Some(value) => value.as_str().and_then(relative),
                };
                if let Some(source) = source {
                    add(
                        &mut dirs,
                        root.join(plugin_root).join(source),
                        plugin.get("skills"),
                    );
                }
            }
        }
    }
    if let Some(manifest) = read(&root.join(".claude-plugin/plugin.json")) {
        add(&mut dirs, root.to_path_buf(), manifest.get("skills"));
    }
    dirs
}

/// The skill named `skill`, preferring its recorded location in the repository.
pub(super) fn locate(checkout: &Path, source: &RemoteSkill, skill: &str) -> Result<PathBuf> {
    if let Some(path) = source.skill_path.as_deref().map(|path| {
        path.trim_matches('/')
            .trim_end_matches("SKILL.md")
            .trim_end_matches('/')
    }) && Path::new(path)
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
        && checkout.join(path).join("SKILL.md").is_file()
    {
        return Ok(checkout.join(path));
    }
    // A recorded subpath can also be the folder the skill was discovered from.
    let root = RemoteSkill {
        skill_path: None,
        ..source.clone()
    };
    let found = match discover(checkout, source, Some(skill)) {
        Ok(found) if !found.is_empty() => found,
        _ => discover(checkout, &root, Some(skill))?,
    };
    found
        .into_iter()
        .next()
        .map(|(skill, _)| skill.path)
        .context(crate::user_error!("errors.repository_skill_missing"))
}

pub(super) type Sources = BTreeMap<String, RemoteSkill>;

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, process::Command};

    use super::*;
    use crate::{SkillLibrary, SkillOrigin};

    fn git(root: &Path, args: &[&str]) {
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
    }

    fn commit(root: &Path) {
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
                "fixture",
            ],
        );
    }

    fn write_skill(dir: &Path, name: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Fixture\n---\n{body}\n"),
        )
        .unwrap();
    }

    fn source(repository: &str, reference: Option<&str>, path: Option<&str>) -> RemoteSkill {
        RemoteSkill {
            repository: repository.into(),
            reference: reference.map(str::to_owned),
            skill_path: path.map(str::to_owned),
        }
    }

    #[test]
    fn parses_skills_cli_source_forms() {
        let gh = "https://github.com/owner/repo";
        let cases = [
            ("owner/repo", source(gh, None, None), None),
            ("owner/repo@pdf", source(gh, None, None), Some("pdf")),
            (
                "owner/repo/skills/pdf",
                source(gh, None, Some("skills/pdf")),
                None,
            ),
            ("github:owner/repo.git", source(gh, None, None), None),
            (
                "owner/repo#dev@pdf",
                source(gh, Some("dev"), None),
                Some("pdf"),
            ),
            (
                "https://skills.sh/owner/repo/pdf",
                source(gh, None, None),
                Some("pdf"),
            ),
            (
                "https://github.com/owner/repo/tree/main/skills/pdf",
                source(gh, Some("main"), Some("skills/pdf")),
                None,
            ),
            (
                "git@github.com:owner/repo.git",
                source("git@github.com:owner/repo.git", None, None),
                None,
            ),
        ];
        for (input, source, skill) in cases {
            assert_eq!(
                parse_target(input).unwrap(),
                Target {
                    source,
                    skill: skill.map(str::to_owned)
                },
                "{input}"
            );
        }
    }

    #[test]
    fn discovers_like_the_skills_cli() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_skill(&root.join("skills/alpha"), "alpha", "");
        write_skill(&root.join("skills/tools/office/beta"), "beta", "");
        write_skill(&root.join("skills/alpha/nested"), "nested", "");
        write_skill(&root.join(".claude/skills/gamma"), "gamma", "");
        write_skill(&root.join(".claude/skills/dupe"), "alpha", "");
        write_skill(&root.join("plugins/p/skills/delta"), "delta", "");
        write_skill(&root.join("node_modules/x/skills/omega"), "omega", "");
        write_skill(&root.join("examples/demo/sample"), "sample", "");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"plugins":[{"name":"p","source":"./plugins/p"}]}"#,
        )
        .unwrap();
        let names = |found: Vec<(ScannedSkill, RemoteSkill)>| {
            found
                .into_iter()
                .map(|(skill, source)| (skill.name, source.skill_path.unwrap_or_default()))
                .collect::<Vec<_>>()
        };
        let all = source("repo", None, None);
        let mut found = names(discover(root, &all, None).unwrap());
        found.sort();
        assert_eq!(
            found,
            [
                ("alpha".into(), "skills/alpha".into()),
                ("beta".into(), "skills/tools/office/beta".into()),
                ("delta".into(), "plugins/p/skills/delta".into()),
                ("gamma".into(), ".claude/skills/gamma".into()),
            ]
        );
        assert_eq!(
            names(discover(root, &all, Some("Gamma")).unwrap()),
            [("gamma".into(), ".claude/skills/gamma".into())]
        );
        // A path to one skill offers only that skill.
        assert_eq!(
            names(discover(root, &source("repo", None, Some("skills/alpha")), None).unwrap()),
            [("alpha".into(), "skills/alpha".into())]
        );
        // Unconventional layouts fall back to a recursive search.
        assert_eq!(
            names(discover(root, &source("repo", None, Some("examples")), None).unwrap()),
            [("sample".into(), "examples/demo/sample".into())]
        );
        assert!(discover(root, &source("repo", None, Some("../x")), None).is_err());
    }

    #[test]
    fn git_scan_imports_updates_and_checks_without_npx() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "--object-format=sha1", "-b", "main"]);
        write_skill(&repo.join("skills/alpha"), "alpha", "v1");
        write_skill(&repo.join("skills/beta"), "beta", "v1");
        commit(&repo);
        let repository = repo.to_str().unwrap().to_owned();

        let scan = super::super::scan_with_git(repository.clone()).unwrap();
        let selected = HashSet::from(["alpha".to_owned()]);
        let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
        scan.import_selected(&mut library, &selected, None).unwrap();
        let record = library.record("alpha").unwrap();
        assert!(matches!(&record.origin, SkillOrigin::Npx { skill, .. } if skill == "alpha"));
        assert_eq!(
            library
                .remote_skill_source(&record)
                .unwrap()
                .skill_path
                .as_deref(),
            Some("skills/alpha")
        );
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 0);

        write_skill(&repo.join("skills/alpha"), "alpha", "v2");
        commit(&repo);
        assert_eq!(super::super::check_updates(&mut library).unwrap(), 1);
        let checkout = tempfile::tempdir().unwrap();
        let path =
            super::super::update_with_git(&library, &record, &repository, "alpha", checkout.path())
                .unwrap();
        assert!(
            fs::read_to_string(path.join("SKILL.md"))
                .unwrap()
                .contains("v2")
        );
    }
}
