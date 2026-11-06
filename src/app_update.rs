//! Updates Kitter itself from the latest GitHub release.
//!
//! The desktop app and the standalone CLI update in place. Network and archive
//! work goes through the system `curl`, `tar` and `hdiutil`, so no HTTP stack
//! is bundled.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE: &str = "https://api.github.com/repos/what1f/kitter/releases/latest";

#[derive(Clone, Debug, Serialize)]
pub struct Release {
    /// Version without the leading `v`.
    pub version: String,
    #[serde(skip)]
    assets: Vec<Asset>,
}

#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum Progress {
    Downloading { done: u64, total: u64 },
    Installing,
}

#[derive(Debug)]
pub struct DesktopUpdate {
    /// Where the new desktop app now lives.
    pub path: PathBuf,
    /// Result of updating the standalone CLI, when one was found.
    pub cli: Option<Result<PathBuf, String>>,
}

pub fn latest_release() -> Result<Release> {
    #[derive(Deserialize)]
    struct Response {
        tag_name: String,
        assets: Vec<Asset>,
    }
    let output = curl()
        .args([
            "--max-time",
            "20",
            "-H",
            "Accept: application/vnd.github+json",
        ])
        .arg(LATEST_RELEASE)
        .output()
        .map_err(|error| check_failed(&error.to_string()))?;
    if !output.status.success() {
        return Err(check_failed(String::from_utf8_lossy(&output.stderr).trim()).into());
    }
    let response: Response =
        serde_json::from_slice(&output.stdout).map_err(|error| check_failed(&error.to_string()))?;
    Ok(Release {
        version: response.tag_name.trim_start_matches('v').to_string(),
        assets: response.assets,
    })
}

pub fn has_update(release: &Release) -> bool {
    is_newer(&release.version, CURRENT_VERSION)
}

pub fn update_cli(release: &Release, mut progress: impl FnMut(Progress)) -> Result<PathBuf> {
    let target = env::current_exe()?.canonicalize()?;
    install_cli(release, &target, &mut progress)?;
    Ok(target)
}

pub fn update_desktop(
    release: &Release,
    mut progress: impl FnMut(Progress),
) -> Result<DesktopUpdate> {
    let asset = release.asset(&desktop_asset_name(&release.version)?)?;
    let work = tempfile::tempdir()?;
    let download = download(asset, work.path(), &mut progress)?;
    progress(Progress::Installing);
    let path = desktop_target()?;
    install_desktop(&download, &path, work.path())?;

    // The two read the same data, so keep a standalone CLI on the same version.
    let cli = standalone_cli()
        .filter(|cli| cli_version(cli).is_none_or(|version| is_newer(&release.version, &version)))
        .map(|cli| {
            install_cli(release, &cli, &mut |_| {})
                .map(|()| cli)
                .map_err(|error| crate::i18n::error_message(&error))
        });
    Ok(DesktopUpdate { path, cli })
}

/// Starts the updated desktop app once this process has exited.
pub fn relaunch(update: &DesktopUpdate) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "while kill -0 \"$0\" 2>/dev/null; do sleep 0.2; done; open \"$1\"",
            ])
            .arg(std::process::id().to_string())
            .arg(&update.path);
        command
    };
    #[cfg(not(target_os = "macos"))]
    let mut command = Command::new(&update.path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

/// Removes what a previous update left behind.
pub fn cleanup_previous() {
    if let Ok(path) = desktop_target() {
        let _ = remove_path(&sibling(&path, "old"));
    }
}

impl Release {
    fn asset(&self, name: &str) -> Result<&Asset> {
        self.assets
            .iter()
            .find(|asset| asset.name == name)
            .ok_or_else(|| crate::user_error!("errors.app_update_unavailable").into())
    }
}

fn install_cli(release: &Release, target: &Path, progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let asset = release.asset(&cli_asset_name(&release.version)?)?;
    let work = tempfile::tempdir()?;
    let archive = download(asset, work.path(), progress)?;
    progress(Progress::Installing);
    let unpacked = work.path().join("cli");
    extract(&archive, &unpacked)?;
    let binary = if cfg!(windows) {
        "kitter.exe"
    } else {
        "kitter"
    };
    replace_file(&unpacked.join(binary), target)
}

#[cfg(target_os = "macos")]
fn install_desktop(dmg: &Path, app: &Path, work: &Path) -> Result<()> {
    let mount = work.join("mount");
    fs::create_dir_all(&mount)?;
    run(Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(dmg))?;
    let staged = sibling(app, "new");
    let result = (|| {
        let _ = remove_path(&staged);
        run(Command::new("ditto")
            .arg(mount.join("Kitter.app"))
            .arg(&staged))
        .map_err(|error| no_permission(app, error))?;
        swap(&staged, app)
    })();
    let _ = Command::new("hdiutil")
        .args(["detach", "-force"])
        .arg(&mount)
        .output();
    if result.is_err() {
        let _ = remove_path(&staged);
    }
    result
}

#[cfg(target_os = "linux")]
fn install_desktop(archive: &Path, binary: &Path, work: &Path) -> Result<()> {
    let unpacked = work.join("desktop");
    extract(archive, &unpacked)?;
    replace_file(&unpacked.join("Kitter").join("Kitter"), binary)
}

#[cfg(windows)]
fn install_desktop(exe: &Path, target: &Path, _work: &Path) -> Result<()> {
    replace_file(exe, target)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn install_desktop(_: &Path, _: &Path, _: &Path) -> Result<()> {
    crate::user_bail!("errors.app_update_unavailable")
}

/// The `.app` bundle on macOS, the executable elsewhere.
fn desktop_target() -> Result<PathBuf> {
    let exe = env::current_exe()?.canonicalize()?;
    if cfg!(target_os = "macos") {
        return exe
            .ancestors()
            .find(|path| path.extension().is_some_and(|ext| ext == "app"))
            .map(Path::to_path_buf)
            .ok_or_else(|| crate::user_error!("errors.app_update_not_installed").into());
    }
    Ok(exe)
}

fn standalone_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "kitter.exe"
    } else {
        "kitter"
    };
    let path = dirs::home_dir()?.join(".local").join("bin").join(name);
    path.is_file().then_some(path)
}

fn cli_version(cli: &Path) -> Option<String> {
    let output = quiet(Command::new(cli)).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace().last().map(str::to_string)
}

fn platform() -> Result<(&'static str, &'static str)> {
    let os = match env::consts::OS {
        os @ ("macos" | "linux" | "windows") => os,
        _ => crate::user_bail!("errors.app_update_unavailable"),
    };
    let arch = match (os, env::consts::ARCH) {
        ("macos", "aarch64") => "arm64",
        (_, "x86_64") => "x86_64",
        _ => crate::user_bail!("errors.app_update_unavailable"),
    };
    Ok((os, arch))
}

fn desktop_asset_name(version: &str) -> Result<String> {
    Ok(match platform()? {
        ("macos", arch) => format!("Kitter-{version}-macos-{arch}.dmg"),
        ("windows", arch) => format!("Kitter-{version}-desktop-windows-{arch}.exe"),
        (os, arch) => format!("Kitter-{version}-desktop-{os}-{arch}.tar.gz"),
    })
}

fn cli_asset_name(version: &str) -> Result<String> {
    let (os, arch) = platform()?;
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    Ok(format!("Kitter-{version}-cli-{os}-{arch}.{ext}"))
}

fn download(asset: &Asset, dir: &Path, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf> {
    let path = dir.join(&asset.name);
    let mut child = curl()
        .args(["--retry", "2", "--connect-timeout", "20", "-o"])
        .arg(&path)
        .arg(&asset.browser_download_url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| download_failed(&error.to_string()))?;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        let done = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        progress(Progress::Downloading {
            done,
            total: asset.size,
        });
        thread::sleep(Duration::from_millis(150));
    };
    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = io::Read::read_to_string(&mut pipe, &mut stderr);
        }
        return Err(download_failed(stderr.trim()).into());
    }
    progress(Progress::Downloading {
        done: asset.size,
        total: asset.size,
    });
    if let Some(expected) = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        let actual = format!("{:x}", Sha256::digest(fs::read(&path)?));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(download_failed("checksum mismatch").into());
        }
    }
    Ok(path)
}

fn extract(archive: &Path, into: &Path) -> Result<()> {
    fs::create_dir_all(into)?;
    run(tar().arg("-xf").arg(archive).arg("-C").arg(into))
}

/// Puts `new` at `target`, keeping the old file until the new one is in place.
fn replace_file(new: &Path, target: &Path) -> Result<()> {
    let staged = sibling(target, "new");
    fs::copy(new, &staged).map_err(|error| no_permission(target, error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
    }
    swap(&staged, target).inspect_err(|_| {
        let _ = fs::remove_file(&staged);
    })
}

/// Renames `staged` over `target`. The old copy moves aside first because a
/// running executable on Windows can be renamed but not overwritten.
fn swap(staged: &Path, target: &Path) -> Result<()> {
    let old = sibling(target, "old");
    let _ = remove_path(&old);
    fs::rename(target, &old).map_err(|error| no_permission(target, error))?;
    if let Err(error) = fs::rename(staged, target) {
        let _ = fs::rename(&old, target);
        return Err(no_permission(target, error));
    }
    // A running Windows executable can't be removed; cleanup_previous gets it later.
    let _ = remove_path(&old);
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{suffix}"))
}

fn remove_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn is_newer(candidate: &str, current: &str) -> bool {
    fn parse(version: &str) -> Option<Vec<u64>> {
        version
            .split(['-', '+'])
            .next()?
            .split('.')
            .map(|part| part.parse().ok())
            .collect()
    }
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}

fn run(command: &mut Command) -> Result<()> {
    let output = command.output().with_context(|| format!("{command:?}"))?;
    if !output.status.success() {
        bail!(
            "{:?}: {}",
            command.get_program(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn curl() -> Command {
    let mut command = quiet(Command::new(system_tool("curl")));
    command
        .args(["-fsSL", "-A"])
        .arg(format!("Kitter/{CURRENT_VERSION}"));
    command
}

fn tar() -> Command {
    quiet(Command::new(system_tool("tar")))
}

/// Windows ships curl and a tar that reads zip files in System32; a Git
/// install on PATH may shadow them with versions that don't.
fn system_tool(name: &str) -> PathBuf {
    #[cfg(windows)]
    if let Some(root) = env::var_os("SystemRoot") {
        let path = PathBuf::from(root)
            .join("System32")
            .join(format!("{name}.exe"));
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from(name)
}

fn quiet(mut command: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command.stdin(Stdio::null());
    command
}

fn check_failed(detail: &str) -> crate::i18n::UserError {
    crate::user_error!("errors.app_update_check_failed").detail(detail.to_string())
}

fn download_failed(detail: &str) -> crate::i18n::UserError {
    crate::user_error!("errors.app_update_download_failed").detail(detail.to_string())
}

fn no_permission(path: &Path, error: impl std::fmt::Display) -> anyhow::Error {
    crate::user_error!("errors.app_update_no_permission", path = path.display())
        .detail(error.to_string())
        .into()
}
