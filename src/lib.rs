#[macro_use]
extern crate rust_i18n;
rust_i18n::i18n!("locales", fallback = "en");
pub mod adoption;
pub mod agent_plugins;
pub mod agents;
pub mod app_update;
pub mod config;
pub mod data_location;
mod directory_link;
pub mod effective_skills;
pub mod i18n;
pub mod library;
pub mod model;
pub mod preset;
pub mod project;
pub mod source;
pub mod tags;
pub mod themes;
mod trigger;

#[cfg(feature = "desktop")]
pub mod assets;
#[cfg(feature = "desktop")]
mod platform;
#[cfg(feature = "desktop")]
pub mod ui;

pub use config::AppConfig;
pub use library::SkillLibrary;
pub use model::*;
