use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: Theme,
    pub library_dir: PathBuf,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default)]
    pub project_activity: BTreeMap<PathBuf, u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::System,
            theme: Theme::System,
            library_dir: app_data_dir().join("skills"),
            recent_projects: vec![],
            project_activity: BTreeMap::new(),
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("KITTER_HOME") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "windows")]
    let base = dirs::data_local_dir();
    #[cfg(not(target_os = "windows"))]
    let base = dirs::data_dir();
    base.unwrap_or_else(|| PathBuf::from(".")).join("Kitter")
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
        let bytes = fs::read(&path).with_context(|| format!("读取配置失败：{}", path.display()))?;
        serde_json::from_slice(&bytes).context("配置格式无效")
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&app_data_dir())
    }

    pub fn save_to(&self, data_dir: &Path) -> Result<()> {
        save_json(&data_dir.join("config.json"), self)
    }

    pub fn remember_project(&mut self, path: PathBuf) {
        let needs_activity = !self.project_activity.contains_key(&path);
        self.recent_projects.retain(|item| item != &path);
        self.recent_projects.insert(0, path.clone());
        self.recent_projects.truncate(8);
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
        self.recent_projects.truncate(8);
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
    use std::path::PathBuf;

    use super::{AppConfig, Language};

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
    }
}
