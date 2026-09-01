use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillOrigin {
    Builtin,
    Npx {
        repository: String,
        skill: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_hash: Option<String>,
    },
    ClaudeMarketplace {
        plugin: String,
        skill: String,
    },
    Git {
        repository: String,
        subdir: Option<String>,
    },
    Local {
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_root: Option<PathBuf>,
    },
    #[default]
    Unknown,
}

impl SkillOrigin {
    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin)
    }

    pub fn source(&self) -> SkillSource {
        match self {
            Self::Builtin => SkillSource::Builtin,
            Self::Npx { repository, .. } => SkillSource::Npx {
                repository: repository.clone(),
            },
            Self::ClaudeMarketplace { plugin, .. } => SkillSource::ClaudeMarketplace {
                plugin: plugin.clone(),
            },
            Self::Git { repository, subdir } => SkillSource::Git {
                repository: repository.clone(),
                subdir: subdir.clone(),
            },
            Self::Local { path, source_root } => SkillSource::Local {
                path: source_root.clone().unwrap_or_else(|| path.clone()),
            },
            Self::Unknown => SkillSource::Unknown,
        }
    }

    pub fn label(&self) -> String {
        self.source().label()
    }

    pub fn identity_key(&self, name: &str) -> String {
        format!("{}::{}", self.source().key(), name)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillSource {
    Builtin,
    Npx {
        repository: String,
    },
    ClaudeMarketplace {
        plugin: String,
    },
    Git {
        repository: String,
        subdir: Option<String>,
    },
    Local {
        path: PathBuf,
    },
    #[default]
    Unknown,
}

impl SkillSource {
    pub fn key(&self) -> String {
        match self {
            Self::Builtin => "builtin".into(),
            Self::Npx { repository } => format!("npx:{repository}"),
            Self::ClaudeMarketplace { plugin } => format!("claude:{plugin}"),
            Self::Git { repository, subdir } => {
                format!("git:{repository}:{}", subdir.as_deref().unwrap_or_default())
            }
            Self::Local { path } => format!("local:{}", path.display()),
            Self::Unknown => "unknown".into(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Builtin => "Kitter".into(),
            Self::Npx { repository } if repository.contains("vercel-labs/skills") => {
                "Vercel Skills".into()
            }
            Self::Npx { repository } | Self::Git { repository, .. } => repository_label(repository),
            Self::ClaudeMarketplace { plugin } => plugin.clone(),
            Self::Local { path } => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "本地导入".into()),
            Self::Unknown => "本地技能".into(),
        }
    }
}

fn repository_label(repository: &str) -> String {
    let value = repository
        .trim_end_matches(".git")
        .strip_prefix("git@github.com:")
        .unwrap_or(repository.trim_end_matches(".git"));
    let parts = value
        .trim_end_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() >= 2 {
        format!("{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        value.to_string()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillSourceRecord {
    pub source: SkillSource,
    #[serde(default)]
    pub discovered_skills: Vec<String>,
    #[serde(default)]
    pub added_skills: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillRecord {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub storage_name: String,
    #[serde(skip)]
    pub description: String,
    #[serde(default)]
    pub origin: SkillOrigin,
    #[serde(default)]
    pub update_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default)]
    pub last_operated_at: u64,
}

impl SkillRecord {
    pub fn identity_key(&self) -> String {
        self.origin.identity_key(&self.name)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillSummary {
    pub record: SkillRecord,
    pub path: PathBuf,
    pub installed_projects: usize,
    pub manual_only: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InstallTarget {
    Universal,
    Codex,
    ClaudeCode,
    Cursor,
    OpenCode,
    Pi,
    Grok,
    Antigravity,
    Droid,
    Copilot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSkill {
    pub name: String,
    pub installations: Vec<ProjectSkillInstallation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSkillInstallation {
    pub target: InstallTarget,
    pub path: PathBuf,
    pub managed: bool,
}
