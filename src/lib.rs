pub mod adoption;
pub mod agents;
pub mod config;
mod directory_link;
pub mod effective_skills;
pub mod library;
pub mod model;
pub mod project;
pub mod source;
pub mod tags;

#[cfg(feature = "desktop")]
pub mod assets;
#[cfg(feature = "desktop")]
mod platform;
#[cfg(feature = "desktop")]
pub mod ui;

pub use config::AppConfig;
pub use library::SkillLibrary;
pub use model::*;
