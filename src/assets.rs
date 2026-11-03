use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::Result;
use gpui::{App, AssetSource, SharedString};

pub struct Assets;

/// Icon files of the active theme package, by icon name.
static THEME_ICONS: RwLock<BTreeMap<String, PathBuf>> = RwLock::new(BTreeMap::new());
static THEME_ICON_GENERATION: AtomicU64 = AtomicU64::new(0);
const THEME_ICON_PREFIX: &str = "theme-icons/";

/// Use `icons` in place of the built-in icons of the same names.
pub fn set_theme_icons(icons: BTreeMap<String, PathBuf>) {
    if let Ok(mut slot) = THEME_ICONS.write() {
        *slot = icons;
    }
    THEME_ICON_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Asset path of the theme's replacement for icon `name`. It changes with
/// every theme switch, so GPUI's icon cache never serves the previous
/// theme's drawing.
pub fn theme_icon_path(name: &str) -> String {
    let generation = THEME_ICON_GENERATION.load(Ordering::Relaxed);
    format!("{THEME_ICON_PREFIX}{generation}/{name}.svg")
}

/// A theme icon for `path`: either a [`theme_icon_path`], or a plain
/// `icons/<name>.svg` that components outside Kitter ask for.
fn theme_icon(path: &str) -> Option<Cow<'static, [u8]>> {
    let file = match path.strip_prefix(THEME_ICON_PREFIX) {
        Some(rest) => rest.split_once('/')?.1,
        None => path.strip_prefix("icons/")?,
    };
    let name = file.strip_suffix(".svg")?;
    let icon = THEME_ICONS.read().ok()?.get(name)?.clone();
    std::fs::read(icon).ok().map(Cow::Owned)
}

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/provider-omp.svg",
        include_bytes!("../assets/icons/provider-omp.svg"),
    ),
    (
        "icons/provider-dimagent.svg",
        include_bytes!("../assets/icons/provider-dimagent.svg"),
    ),
    (
        "icons/sparkle.svg",
        include_bytes!("../assets/icons/sparkle.svg"),
    ),
    (
        "icons/layers.svg",
        include_bytes!("../assets/icons/layers.svg"),
    ),
    (
        "icons/package.svg",
        include_bytes!("../assets/icons/package.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../assets/icons/folder-open.svg"),
    ),
    (
        "icons/pencil.svg",
        include_bytes!("../assets/icons/pencil.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/sidebar.svg",
        include_bytes!("../assets/icons/sidebar.svg"),
    ),
    (
        "icons/rotate-cw.svg",
        include_bytes!("../assets/icons/rotate-cw.svg"),
    ),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/trash.svg",
        include_bytes!("../assets/icons/trash.svg"),
    ),
    (
        "icons/download.svg",
        include_bytes!("../assets/icons/download.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    ("icons/hash.svg", include_bytes!("../assets/icons/hash.svg")),
    ("icons/hand.svg", include_bytes!("../assets/icons/hand.svg")),
    (
        "icons/clock-3.svg",
        include_bytes!("../assets/icons/clock-3.svg"),
    ),
    ("icons/file.svg", include_bytes!("../assets/icons/file.svg")),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../assets/icons/chevron-up.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
    (
        "icons/crown.svg",
        include_bytes!("../assets/icons/crown.svg"),
    ),
    (
        "icons/house.svg",
        include_bytes!("../assets/icons/house.svg"),
    ),
    (
        "icons/provider-amp.svg",
        include_bytes!("../assets/icons/provider-amp.svg"),
    ),
    (
        "icons/provider-claude.svg",
        include_bytes!("../assets/icons/provider-claude.svg"),
    ),
    (
        "icons/provider-cursor.svg",
        include_bytes!("../assets/icons/provider-cursor.svg"),
    ),
    (
        "icons/provider-cursor.dark.svg",
        include_bytes!("../assets/icons/provider-cursor.dark.svg"),
    ),
    (
        "icons/provider-gemini.svg",
        include_bytes!("../assets/icons/provider-gemini.svg"),
    ),
    (
        "icons/provider-copilot.svg",
        include_bytes!("../assets/icons/provider-copilot.svg"),
    ),
    (
        "icons/provider-codex.svg",
        include_bytes!("../assets/icons/provider-codex.svg"),
    ),
    (
        "icons/provider-opencode.svg",
        include_bytes!("../assets/icons/provider-opencode.svg"),
    ),
    (
        "icons/provider-opencode.dark.svg",
        include_bytes!("../assets/icons/provider-opencode.dark.svg"),
    ),
    (
        "icons/provider-pi.svg",
        include_bytes!("../assets/icons/provider-pi.svg"),
    ),
    (
        "icons/provider-pi.dark.svg",
        include_bytes!("../assets/icons/provider-pi.dark.svg"),
    ),
    (
        "icons/provider-grok.svg",
        include_bytes!("../assets/icons/provider-grok.svg"),
    ),
    (
        "icons/provider-grok.dark.svg",
        include_bytes!("../assets/icons/provider-grok.dark.svg"),
    ),
    (
        "icons/provider-openclaw.svg",
        include_bytes!("../assets/icons/provider-openclaw.svg"),
    ),
    (
        "icons/provider-hermes.svg",
        include_bytes!("../assets/icons/provider-hermes.svg"),
    ),
    (
        "icons/provider-hermes.dark.svg",
        include_bytes!("../assets/icons/provider-hermes.dark.svg"),
    ),
    (
        "icons/provider-droid.svg",
        include_bytes!("../assets/icons/provider-droid.svg"),
    ),
    (
        "icons/provider-droid.dark.svg",
        include_bytes!("../assets/icons/provider-droid.dark.svg"),
    ),
    (
        "icons/provider-antigravity.svg",
        include_bytes!("../assets/icons/provider-antigravity.svg"),
    ),
    (
        "icons/provider-trae.svg",
        include_bytes!("../assets/icons/provider-trae.svg"),
    ),
    (
        "icons/github.svg",
        include_bytes!("../assets/icons/github.svg"),
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(icon) = theme_icon(path) {
            return Ok(Some(icon));
        }
        // GPUI Component's bundled Lucide assets take precedence over legacy local aliases.
        if let Ok(Some(asset)) = gpui_component_assets::Assets.load(path) {
            return Ok(Some(asset));
        }

        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        for (name, _) in ICONS.iter().filter(|(name, _)| name.starts_with(path)) {
            let name = SharedString::from(*name);
            if !assets.contains(&name) {
                assets.push(name);
            }
        }
        Ok(assets)
    }
}

pub fn register_fonts(cx: &App) -> Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf").as_slice()),
    ])?;
    Ok(())
}
