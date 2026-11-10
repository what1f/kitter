use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    System,
    ZhCn,
    En,
}

impl Language {
    pub fn system() -> Self {
        static SYSTEM_LANGUAGE: OnceLock<Language> = OnceLock::new();
        *SYSTEM_LANGUAGE.get_or_init(Self::detect_system)
    }

    fn detect_system() -> Self {
        let locale = sys_locale::get_locale()
            .or_else(|| std::env::var("LANG").ok())
            .unwrap_or_default();
        Self::from_locale(&locale)
    }

    fn from_locale(locale: &str) -> Self {
        let locale = locale.to_ascii_lowercase();
        if locale.starts_with("zh") {
            Self::ZhCn
        } else {
            Self::En
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Theme of the desktop app when none is picked: a built-in id or the file
/// stem of a theme under `<data dir>/themes`. Independent of light / dark.
pub const DEFAULT_THEME_STYLE: &str = "standard";

fn default_theme_style() -> String {
    DEFAULT_THEME_STYLE.into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_theme_style")]
    pub theme_style: String,
    pub library_dir: PathBuf,
    #[serde(default)]
    // Persist every known project; activity determines display order, not retention.
    pub recent_projects: Vec<PathBuf>,
    #[serde(default)]
    pub project_activity: BTreeMap<PathBuf, u64>,
    #[serde(default)]
    pub collapsed_skill_groups: BTreeSet<String>,
    #[serde(default)]
    pub collapsed_skill_groups_initialized: bool,
    /// The stored library path when loading fell back to another one, as
    /// `(fallback, stored)`. Saving keeps the stored path so a drive that is
    /// only missing for now does not lose the setting.
    #[serde(skip)]
    pub(crate) library_fallback: Option<(PathBuf, PathBuf)>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::System,
            theme: Theme::System,
            theme_style: default_theme_style(),
            library_dir: app_data_dir().join("skills"),
            recent_projects: vec![],
            project_activity: BTreeMap::new(),
            collapsed_skill_groups: BTreeSet::new(),
            collapsed_skill_groups_initialized: false,
            library_fallback: None,
        }
    }
}

/// Name of the file in the default data folder that points to a chosen one.
pub(crate) const LOCATION_FILE: &str = "location.json";

#[derive(Serialize, Deserialize)]
struct Location {
    path: PathBuf,
}

/// The platform's data folder. It also holds the pointer to a chosen folder.
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = dirs::data_local_dir();
    #[cfg(not(target_os = "windows"))]
    let base = dirs::data_dir();
    base.unwrap_or_else(|| PathBuf::from(".")).join("Kitter")
}

fn data_dir_cache() -> &'static RwLock<Option<PathBuf>> {
    static CACHE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

/// `KITTER_HOME`, else the chosen folder, else the platform default. Resolved
/// once per process, so a running app never switches folders midway.
pub fn app_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("KITTER_HOME") {
        return PathBuf::from(path);
    }
    // Tests must not read or write the user's real library, agent plugins included.
    if cfg!(test) {
        return std::env::temp_dir().join(format!("kitter-test-{}", std::process::id()));
    }
    if let Some(path) = data_dir_cache().read().unwrap().clone() {
        return path;
    }
    let default = default_data_dir();
    let path = read_location(&default).unwrap_or(default);
    *data_dir_cache().write().unwrap() = Some(path.clone());
    path
}

pub(crate) fn read_location(default_dir: &Path) -> Option<PathBuf> {
    let bytes = fs::read(default_dir.join(LOCATION_FILE)).ok()?;
    let location = serde_json::from_slice::<Location>(&bytes).ok()?;
    location.path.is_absolute().then_some(location.path)
}

/// Point the default folder at `path`, or drop the pointer when it is the default.
pub(crate) fn write_location(default_dir: &Path, path: &Path) -> Result<()> {
    let pointer = default_dir.join(LOCATION_FILE);
    if path == default_dir {
        match fs::remove_file(&pointer) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    } else {
        save_json(
            &pointer,
            &Location {
                path: path.to_path_buf(),
            },
        )?;
    }
    Ok(())
}

pub(crate) fn set_app_data_dir(path: &Path) {
    *data_dir_cache().write().unwrap() = Some(path.to_path_buf());
}

/// The library lives in `<data>/skills`; older configs store it as an absolute
/// path. Follow the data folder when it was copied or moved by hand.
fn resolve_library_dir(data_dir: &Path, stored: PathBuf) -> PathBuf {
    if stored.is_relative() {
        return data_dir.join(stored);
    }
    let own = data_dir.join("skills");
    if stored == own || !own.is_dir() {
        return stored;
    }
    let other_data_folder = stored.file_name().is_some_and(|name| name == "skills")
        && stored
            .parent()
            .is_some_and(|parent| parent.join("config.json").is_file());
    if !stored.exists() || other_data_folder {
        own
    } else {
        stored
    }
}

impl AppConfig {
    pub fn path() -> PathBuf {
        app_data_dir().join("config.json")
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&app_data_dir())
    }

    pub fn load_from(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("config.json");
        if !path.exists() {
            return Ok(Self {
                library_dir: data_dir.join("skills"),
                ..Self::default()
            });
        }
        let bytes = fs::read(&path)
            .with_context(|| t!("errors.config_read_failed", path = path.display()).to_string())?;
        let mut config: Self =
            serde_json::from_slice(&bytes).context(t!("errors.config_invalid").to_string())?;
        let stored = config.library_dir.clone();
        config.library_dir = resolve_library_dir(data_dir, stored.clone());
        if config.library_dir != stored && stored.is_absolute() {
            config.library_fallback = Some((config.library_dir.clone(), stored));
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&app_data_dir())
    }

    // The path stays absolute so older versions can still read it; loading
    // falls back to the folder's own `skills` after the folder is moved.
    pub fn save_to(&self, data_dir: &Path) -> Result<()> {
        match &self.library_fallback {
            Some((fallback, stored)) if fallback == &self.library_dir => {
                let mut config = self.clone();
                config.library_dir = stored.clone();
                save_json(&data_dir.join("config.json"), &config)
            }
            _ => save_json(&data_dir.join("config.json"), self),
        }
    }

    pub fn remember_project(&mut self, path: PathBuf) {
        let needs_activity = !self.project_activity.contains_key(&path);
        self.recent_projects.retain(|item| item != &path);
        self.recent_projects.insert(0, path.clone());
        if needs_activity {
            self.touch_project(&path);
        } else {
            self.prune_project_activity();
        }
    }

    pub fn project_paths(&self) -> Vec<PathBuf> {
        let mut projects = self
            .recent_projects
            .iter()
            .cloned()
            .enumerate()
            .collect::<Vec<_>>();
        projects.sort_by(|(left_index, left), (right_index, right)| {
            self.project_activity
                .get(right)
                .copied()
                .unwrap_or_default()
                .cmp(&self.project_activity.get(left).copied().unwrap_or_default())
                .then_with(|| left_index.cmp(right_index))
        });
        projects.into_iter().map(|(_, path)| path).collect()
    }

    pub fn touch_project(&mut self, path: &Path) {
        let path = path.to_path_buf();
        if !self.recent_projects.iter().any(|project| project == &path) {
            self.recent_projects.insert(0, path.clone());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();
        let next = self
            .project_activity
            .values()
            .copied()
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        self.project_activity.insert(path, now.max(next));
        self.prune_project_activity();
    }

    pub fn remove_project(&mut self, path: &Path) {
        self.recent_projects.retain(|project| project != path);
        self.project_activity.remove(path);
    }

    fn prune_project_activity(&mut self) {
        self.project_activity
            .retain(|path, _| self.recent_projects.iter().any(|project| project == path));
    }
}

pub fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_missing_library_is_kept_in_the_saved_config() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        std::fs::create_dir_all(data.join("skills")).unwrap();
        let offline = temp.path().join("unmounted/skills");
        std::fs::write(
            data.join("config.json"),
            serde_json::to_vec(&serde_json::json!({"library_dir": offline})).unwrap(),
        )
        .unwrap();

        let mut config = AppConfig::load_from(&data).unwrap();
        assert_eq!(config.library_dir, data.join("skills"));
        config.save_to(&data).unwrap();
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(data.join("config.json")).unwrap()).unwrap();
        assert_eq!(stored["library_dir"], offline.to_string_lossy().as_ref());

        let chosen = temp.path().join("chosen");
        std::fs::create_dir_all(&chosen).unwrap();
        config.library_dir = chosen.clone();
        config.save_to(&data).unwrap();
        assert_eq!(AppConfig::load_from(&data).unwrap().library_dir, chosen);
    }

    use std::path::PathBuf;

    use super::{AppConfig, Language};

    #[test]
    fn theme_style_defaults_to_standard_and_round_trips() {
        let old: AppConfig =
            serde_json::from_str(r#"{"theme":"dark","library_dir":"/tmp/skills"}"#).unwrap();
        assert_eq!(old.theme_style, "standard");

        let config = AppConfig {
            theme_style: "solid_glass".into(),
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""theme_style":"solid_glass""#));
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.theme_style, "solid_glass");
    }

    #[test]
    fn resolves_chinese_locale_variants() {
        for locale in ["zh-CN", "zh-Hans-CN", "zh_TW", "ZH-hant"] {
            assert_eq!(Language::from_locale(locale), Language::ZhCn);
        }
    }

    #[test]
    fn defaults_non_chinese_locales_to_english() {
        for locale in ["en-US", "ja-JP", "", "C"] {
            assert_eq!(Language::from_locale(locale), Language::En);
        }
    }

    #[test]
    fn projects_sort_by_activity_and_keep_path_order_as_tie_breaker() {
        let mut config = AppConfig::default();
        let first = PathBuf::from("/tmp/first");
        let second = PathBuf::from("/tmp/second");
        let third = PathBuf::from("/tmp/third");
        config.recent_projects = vec![first.clone(), second.clone(), third.clone()];
        config.project_activity.insert(first.clone(), 10);
        config.project_activity.insert(second.clone(), 30);
        config.project_activity.insert(third.clone(), 20);

        assert_eq!(config.project_paths(), vec![second, third, first]);
    }

    #[test]
    fn touching_a_project_updates_activity_without_reordering_unrelated_projects() {
        let mut config = AppConfig::default();
        let first = PathBuf::from("/tmp/first");
        let second = PathBuf::from("/tmp/second");
        config.remember_project(first.clone());
        config.remember_project(second.clone());
        let before = config.project_activity[&first];
        config.touch_project(&first);

        assert!(config.project_activity[&first] > before);
        assert_eq!(config.project_paths(), vec![first, second]);
    }

    #[test]
    fn project_activity_is_optional_in_older_configs_and_round_trips() {
        let mut config = AppConfig::default();
        let project = PathBuf::from("/tmp/project");
        config.touch_project(&project);

        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.project_activity, config.project_activity);

        let older = format!(
            r#"{{"language":"system","theme":"system","library_dir":"/tmp/skills","recent_projects":["{}"]}}"#,
            project.display()
        );
        let restored: AppConfig = serde_json::from_str(&older).unwrap();
        assert!(restored.project_activity.is_empty());
        assert!(restored.collapsed_skill_groups.is_empty());
        assert!(!restored.collapsed_skill_groups_initialized);
    }

    #[test]
    fn collapsed_skill_groups_round_trip_and_default_when_missing() {
        let mut config = AppConfig::default();
        config.collapsed_skill_groups.insert("group-a".into());
        config.collapsed_skill_groups_initialized = true;

        let restored: AppConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            restored.collapsed_skill_groups,
            config.collapsed_skill_groups
        );
        assert!(restored.collapsed_skill_groups_initialized);

        let older = r#"{"language":"system","theme":"system","library_dir":"/tmp/skills"}"#;
        let restored: AppConfig = serde_json::from_str(older).unwrap();
        assert!(restored.collapsed_skill_groups.is_empty());
        assert!(!restored.collapsed_skill_groups_initialized);
    }
}
