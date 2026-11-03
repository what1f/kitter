//! Desktop themes: the built-in themes and user theme packages.
//!
//! A theme package is a folder `<data dir>/themes/<id>/` holding
//! `theme.json` plus optional `icons/*.svg` and `fonts/*.ttf|otf|ttc`; a lone
//! `<id>.json` works too. Every key is optional and overrides the theme named
//! by `extends` (`standard` by default):
//!
//! ```json
//! {
//!   "name": "Ocean",
//!   "extends": "solid_glass",
//!   "shape": { "radius_md": 6 },
//!   "light": { "colors": { "accent": "#0a7cff", "focus": "accent" } },
//!   "materials": { "pane": { "shadows": ["0 1 2 0 shade"] } }
//! }
//! ```
//!
//! The built-in themes are themselves theme files (`resources/themes/`), so
//! `standard.json` lists every key. See `docs/themes.md` for the format.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::config::app_data_dir;

pub const STANDARD: &str = "standard";
pub const SOLID_GLASS: &str = "solid_glass";
pub const BUILTIN_THEMES: [&str; 2] = [STANDARD, SOLID_GLASS];
const BUILTIN_JSON: [(&str, &str); 2] = [
    (STANDARD, include_str!("../resources/themes/standard.json")),
    (
        SOLID_GLASS,
        include_str!("../resources/themes/solid_glass.json"),
    ),
];

/// File name of a theme package's manifest.
pub const MANIFEST: &str = "theme.json";
pub const ICONS_DIR: &str = "icons";
pub const FONTS_DIR: &str = "fonts";
pub const IMAGES_DIR: &str = "images";
/// Sub-folder of `icons/` for icons drawn in their own colours.
pub const COLOR_ICONS_DIR: &str = "color";
const FONT_EXTENSIONS: [&str; 3] = ["ttf", "otf", "ttc"];
pub const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
/// How many themes an `extends` chain may hold before it counts as a loop.
const MAX_CHAIN: usize = 8;

const ROOT_KEYS: [&str; 10] = [
    "$schema",
    "name",
    "extends",
    "font",
    "text",
    "shape",
    "size",
    "materials",
    "light",
    "dark",
];
pub const FONT_KEYS: [&str; 4] = ["ui", "mono", "medium", "semibold"];
pub const TEXT_KEYS: [&str; 6] = ["caption", "small", "body", "emphasis", "title", "display"];
pub const SHAPE_KEYS: [&str; 8] = [
    "radius_sm",
    "radius_md",
    "radius_lg",
    "radius_xl",
    "radius_modal",
    "radius_pill",
    "border",
    "divider",
];
pub const SIZE_KEYS: [&str; 16] = [
    "control_sm",
    "control_md",
    "row",
    "row_double",
    "row_setting",
    "pane_header",
    "card_header",
    "file_bar",
    "tab_bar",
    "segment",
    "chip",
    "badge",
    "avatar",
    "icon",
    "icon_sm",
    "icon_xs",
];
const MODE_KEYS: [&str; 2] = ["colors", "materials"];
const MATERIAL_KEYS: [&str; 5] = ["fill", "border", "shadows", "texture", "frame"];
const TEXTURE_KEYS: [&str; 4] = ["image", "size", "opacity", "pixelated"];
const FRAME_KEYS: [&str; 6] = ["image", "slice", "width", "repeat", "fill", "pixelated"];
/// Written instead of a texture or frame to remove an inherited one.
const NONE: &str = "none";

/// Colour keys every theme defines, in display order.
pub const COLOR_KEYS: [&str; 27] = [
    "window",
    "window_border",
    "base",
    "sidebar",
    "pane",
    "surface",
    "elevated",
    "avatar",
    "raised",
    "hover",
    "selected",
    "border",
    "border_strong",
    "text",
    "sidebar_text",
    "secondary",
    "muted",
    "accent",
    "focus",
    "primary",
    "on_primary",
    "success",
    "warning",
    "danger",
    "on_danger",
    "danger_soft",
    "overlay",
];

/// Colour keys a theme may leave out, with the key each falls back to.
/// Switch colours without a fallback keep the component library's own.
pub const OPTIONAL_COLOR_KEYS: [(&str, Option<&str>); 5] = [
    ("dialog_base", Some("base")),
    ("dialog_pane", Some("pane")),
    ("dialog_surface", Some("surface")),
    ("switch_track", None),
    ("switch_thumb", None),
];

/// Surface materials, in display order.
pub const MATERIALS: [&str; 12] = [
    "pane", "popover", "dialog", "control", "state", "primary", "field", "well", "window",
    "sidebar", "content", "column",
];

/// Materials drawn with a border. Their inset shadows start inside the
/// border, as in CSS.
pub const BORDERED_MATERIALS: [&str; 6] =
    ["pane", "popover", "dialog", "control", "primary", "field"];

/// Icons a theme package can replace with `icons/<name>.svg`.
pub const ICON_NAMES: [&str; 27] = [
    "check",
    "chevron-down",
    "chevron-right",
    "chevron-up",
    "clock-3",
    "crown",
    "download",
    "ellipsis",
    "file",
    "folder",
    "folder-open",
    "hand",
    "hash",
    "house",
    "layers",
    "loader-circle",
    "package",
    "pencil",
    "plus",
    "rotate-cw",
    "search",
    "settings",
    "sidebar",
    "sparkle",
    "trash",
    "triangle-alert",
    "x",
];

pub type Numbers = BTreeMap<&'static str, f32>;

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    /// `0xRRGGBBAA`.
    pub color: u32,
    pub inset: bool,
}

/// A material fill: one colour or a two-stop linear gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Solid(u32),
    /// CSS angle in degrees (`180` runs top to bottom); stops are
    /// `(colour, position 0..=1)`.
    Gradient {
        angle: f32,
        from: (u32, f32),
        to: (u32, f32),
    },
}

/// An image tiled over a surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    /// File name in the package's `images/` folder.
    pub image: String,
    /// Tile width in pixels; `None` uses the image's own size.
    pub size: Option<f32>,
    pub opacity: f32,
    /// Scale with hard pixel edges.
    pub pixelated: bool,
}

/// A nine-slice image drawn over a surface's outline.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// File name in the package's `images/` folder.
    pub image: String,
    /// Insets into the image, in image pixels: top, right, bottom, left.
    pub slice: [f32; 4],
    /// Drawn width of each edge, in pixels: top, right, bottom, left.
    pub width: [f32; 4],
    /// Tile the edges and the centre instead of stretching them.
    pub repeat: bool,
    /// Draw the centre of the image as the surface's fill.
    pub fill: bool,
    pub pixelated: bool,
}

/// How a material paints a surface. `None` keeps the component's default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialStyle {
    pub fill: Option<Paint>,
    pub border: Option<u32>,
    pub shadows: Option<Vec<Shadow>>,
    pub texture: Option<Texture>,
    pub frame: Option<Frame>,
}

/// Colours and materials of one colour mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub dark: bool,
    /// Every colour as `0xRRGGBBAA`: [`COLOR_KEYS`], the optional keys the
    /// theme sets or falls back to, and its own named colours.
    pub colors: BTreeMap<String, u32>,
    pub materials: BTreeMap<&'static str, MaterialStyle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub ui: String,
    pub mono: String,
    pub medium: u16,
    pub semibold: u16,
}

/// Every design token of a resolved theme.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub font: Font,
    pub text: Numbers,
    pub shape: Numbers,
    pub size: Numbers,
    pub light: Mode,
    pub dark: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    Builtin,
    File(PathBuf),
    /// A theme package folder.
    Package(PathBuf),
}

impl ThemeSource {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Builtin => None,
            Self::File(path) | Self::Package(path) => Some(path),
        }
    }
}

/// A theme with its `extends` chain merged in.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub source: ThemeSource,
    /// The `extends` value of this theme's own file.
    pub extends: Option<String>,
    /// Every key of the chain merged, colour names kept as written.
    pub document: Map<String, Value>,
    pub tokens: Tokens,
    /// Replacement icon files by icon name, the nearest theme winning.
    pub icons: BTreeMap<String, PathBuf>,
    /// Replacement icons drawn in their own colours, by icon name.
    pub color_icons: BTreeMap<String, PathBuf>,
    /// Image files by file name, for textures and frames.
    pub images: BTreeMap<String, PathBuf>,
    /// Font files of every package in the chain.
    pub fonts: Vec<PathBuf>,
}

/// A user theme next to what went wrong reading it.
#[derive(Clone, Debug)]
pub struct ThemeFile {
    pub id: String,
    /// The package folder or the `.json` file.
    pub path: PathBuf,
    /// `Err` when the file can't be read or isn't a JSON object.
    pub theme: Result<Theme, String>,
    /// Keys and values that were ignored.
    pub problems: Vec<String>,
}

pub fn themes_dir() -> PathBuf {
    app_data_dir().join("themes")
}

pub fn builtin_name(id: &str) -> Option<String> {
    match id {
        STANDARD => Some(t!("settings.style_standard").to_string()),
        SOLID_GLASS => Some(t!("settings.style_solid_glass").to_string()),
        _ => None,
    }
}

pub fn builtin(id: &str) -> Option<Theme> {
    static BUILTINS: OnceLock<Vec<Theme>> = OnceLock::new();
    let themes = BUILTINS.get_or_init(|| {
        BUILTIN_THEMES
            .into_iter()
            .map(|id| {
                let layer = builtin_layer(id).expect("built-in theme");
                resolve_layers(Path::new(""), layer).0
            })
            .collect()
    });
    let mut theme = themes.iter().find(|theme| theme.id == id)?.clone();
    theme.name = builtin_name(id)?;
    Some(theme)
}

/// Whether `id` can name a user theme: no path separators, no leading dot,
/// and not a built-in id.
pub fn is_file_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && !id.contains(['/', '\\'])
        && !BUILTIN_THEMES.contains(&id)
}

/// User themes in `dir`, sorted by name. Themes that fail to load are kept
/// so callers can report them. A package folder hides a `.json` file of the
/// same id.
pub fn list_files_in(dir: &Path) -> Vec<ThemeFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut ids = BTreeSet::new();
    for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
        let id = if path.is_dir() && path.join(MANIFEST).is_file() {
            path.file_name()
        } else if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            path.file_stem()
        } else {
            None
        };
        if let Some(id) = id.and_then(|id| id.to_str()).filter(|id| is_file_id(id)) {
            ids.insert(id.to_string());
        }
    }
    let mut files = ids
        .iter()
        .filter_map(|id| load_in(dir, id))
        .collect::<Vec<_>>();
    files.sort_by_key(|file| {
        let name = file.theme.as_ref().map_or(&file.id, |theme| &theme.name);
        (name.to_lowercase(), file.id.clone())
    });
    files
}

/// Built-in themes, then readable user themes sorted by name.
pub fn list_in(dir: &Path) -> Vec<Theme> {
    BUILTIN_THEMES
        .into_iter()
        .filter_map(builtin)
        .chain(
            list_files_in(dir)
                .into_iter()
                .filter_map(|file| file.theme.ok()),
        )
        .collect()
}

/// A user theme in `dir`, or `None` when there is none by that id.
pub fn load_in(dir: &Path, id: &str) -> Option<ThemeFile> {
    let source = locate(dir, id)?;
    let path = source.path().expect("user theme path").to_path_buf();
    Some(load_source(dir, id, source, path))
}

/// Read a theme file or package outside the themes folder, resolving its
/// `extends` against `dir`.
pub fn load_path(dir: &Path, path: &Path) -> ThemeFile {
    let package = path.is_dir();
    let id = if package {
        path.file_name()
    } else {
        path.file_stem()
    }
    .and_then(|id| id.to_str())
    .unwrap_or_default()
    .to_string();
    let source = if package {
        ThemeSource::Package(path.to_path_buf())
    } else {
        ThemeSource::File(path.to_path_buf())
    };
    load_source(dir, &id, source, path.to_path_buf())
}

fn load_source(dir: &Path, id: &str, source: ThemeSource, path: PathBuf) -> ThemeFile {
    let (theme, problems) = match read_layer(id, source) {
        Ok(layer) => {
            let (theme, problems) = resolve_layers(dir, layer);
            (Ok(theme), problems)
        }
        Err(error) => (Err(error), vec![]),
    };
    ThemeFile {
        id: id.into(),
        path,
        theme,
        problems,
    }
}

/// A theme by id; built-in ids win over user themes.
pub fn find_in(dir: &Path, id: &str) -> Result<Theme> {
    if let Some(theme) = builtin(id) {
        return Ok(theme);
    }
    let Some(file) = load_in(dir, id) else {
        crate::user_bail!("errors.theme_not_found", id = id);
    };
    file.theme
        .map_err(|error| crate::user_error!("errors.theme_invalid", id = id, error = error).into())
}

/// The theme for `id`, falling back to the standard theme when it's
/// missing or unreadable.
pub fn resolve_in(dir: &Path, id: &str) -> Theme {
    find_in(dir, id).unwrap_or_else(|_| builtin(STANDARD).expect("standard theme"))
}

/// One theme file before its `extends` chain is merged in.
struct Layer {
    id: String,
    name: Option<String>,
    extends: Option<String>,
    /// The valid keys of the file.
    doc: Map<String, Value>,
    source: ThemeSource,
    icons: BTreeMap<String, PathBuf>,
    color_icons: BTreeMap<String, PathBuf>,
    images: BTreeMap<String, PathBuf>,
    fonts: Vec<PathBuf>,
    problems: Vec<String>,
}

fn builtin_layer(id: &str) -> Option<Layer> {
    let (_, text) = BUILTIN_JSON
        .into_iter()
        .find(|(builtin, _)| *builtin == id)?;
    Some(parse_layer(id, text, ThemeSource::Builtin).expect("built-in theme"))
}

/// Where the user theme `id` lives in `dir`; a package wins over a file.
fn locate(dir: &Path, id: &str) -> Option<ThemeSource> {
    if !is_file_id(id) {
        return None;
    }
    let package = dir.join(id);
    if package.join(MANIFEST).is_file() {
        return Some(ThemeSource::Package(package));
    }
    let file = dir.join(format!("{id}.json"));
    file.is_file().then_some(ThemeSource::File(file))
}

fn read_layer(id: &str, source: ThemeSource) -> Result<Layer, String> {
    let manifest = match &source {
        ThemeSource::Builtin => {
            return builtin_layer(id).ok_or_else(|| t!("themes.not_object").to_string());
        }
        ThemeSource::File(path) => path.clone(),
        ThemeSource::Package(dir) => dir.join(MANIFEST),
    };
    let text = fs::read_to_string(&manifest).map_err(|error| error.to_string())?;
    let mut layer = parse_layer(id, &text, source)?;
    if let ThemeSource::Package(dir) = layer.source.clone() {
        read_package_assets(&dir, &mut layer);
    }
    Ok(layer)
}

fn read_package_assets(dir: &Path, layer: &mut Layer) {
    let icons = dir.join(ICONS_DIR);
    for (folder, prefix, color) in [
        (icons.clone(), "", false),
        (icons.join(COLOR_ICONS_DIR), "color/", true),
    ] {
        for path in sorted_files(&folder) {
            if path.extension().is_none_or(|ext| ext != "svg") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if !ICON_NAMES.contains(&name) {
                let name = format!("{prefix}{name}");
                layer
                    .problems
                    .push(t!("themes.unknown_icon", name = name).to_string());
            } else if color {
                layer.color_icons.insert(name.to_string(), path.clone());
            } else {
                layer.icons.insert(name.to_string(), path.clone());
            }
        }
    }
    for path in sorted_files(&dir.join(IMAGES_DIR)) {
        let image = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()));
        if let (true, Some(name)) = (image, path.file_name().and_then(|name| name.to_str())) {
            layer.images.insert(name.to_string(), path.clone());
        }
    }
    layer.fonts = sorted_files(&dir.join(FONTS_DIR))
        .into_iter()
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| FONT_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        })
        .collect();
}

fn sorted_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut files = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// Parse one theme file. Invalid values and unknown keys are skipped and
/// reported; only text that isn't a JSON object fails.
fn parse_layer(id: &str, text: &str, source: ThemeSource) -> Result<Layer, String> {
    let root = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(root)) => root,
        Ok(_) => return Err(t!("themes.not_object").to_string()),
        Err(error) => return Err(error.to_string()),
    };
    let mut problems = Vec::new();
    let mut doc = Map::new();
    for (key, value) in &root {
        let clean = match key.as_str() {
            "$schema" | "name" | "extends" => None,
            "font" => clean_font(value, &mut problems),
            "text" => clean_numbers("text", &TEXT_KEYS, value, &mut problems),
            "shape" => clean_numbers("shape", &SHAPE_KEYS, value, &mut problems),
            "size" => clean_numbers("size", &SIZE_KEYS, value, &mut problems),
            "materials" => clean_materials("materials", value, &mut problems),
            "light" | "dark" => clean_mode(key, value, &mut problems),
            _ => {
                problems.push(t!("themes.unknown_key", key = key).to_string());
                None
            }
        };
        if let Some(clean) = clean {
            doc.insert(key.clone(), clean);
        }
    }
    let name = match root.get("name") {
        None => None,
        Some(Value::String(name)) if !name.trim().is_empty() => Some(name.trim().to_string()),
        Some(_) => {
            problems.push(t!("themes.invalid_name").to_string());
            None
        }
    };
    let extends = match root.get("extends") {
        None => None,
        Some(Value::String(base)) if !base.trim().is_empty() => Some(base.trim().to_string()),
        Some(_) => {
            problems.push(t!("themes.invalid_extends").to_string());
            None
        }
    };
    Ok(Layer {
        id: id.into(),
        name,
        extends,
        doc,
        source,
        icons: BTreeMap::new(),
        color_icons: BTreeMap::new(),
        images: BTreeMap::new(),
        fonts: vec![],
        problems,
    })
}

fn as_object<'a>(
    path: &str,
    value: &'a Value,
    problems: &mut Vec<String>,
) -> Option<&'a Map<String, Value>> {
    let object = value.as_object();
    if object.is_none() {
        problems.push(t!("themes.not_an_object", key = path).to_string());
    }
    object
}

fn unknown(path: &str, key: &str, problems: &mut Vec<String>) {
    problems.push(t!("themes.unknown_key", key = format!("{path}.{key}")).to_string());
}

fn invalid(path: &str, key: &str, problems: &mut Vec<String>) {
    problems.push(t!("themes.invalid_value", key = format!("{path}.{key}")).to_string());
}

fn invalid_color(path: &str, key: &str, problems: &mut Vec<String>) {
    problems.push(t!("themes.invalid_color", key = format!("{path}.{key}")).to_string());
}

fn clean_numbers(
    path: &str,
    keys: &[&str],
    value: &Value,
    problems: &mut Vec<String>,
) -> Option<Value> {
    let object = as_object(path, value, problems)?;
    let mut clean = Map::new();
    for (key, value) in object {
        if !keys.contains(&key.as_str()) {
            unknown(path, key, problems);
        } else if value
            .as_f64()
            .is_some_and(|number| number.is_finite() && (0.0..=1000.0).contains(&number))
        {
            clean.insert(key.clone(), value.clone());
        } else {
            invalid(path, key, problems);
        }
    }
    Some(Value::Object(clean))
}

fn clean_font(value: &Value, problems: &mut Vec<String>) -> Option<Value> {
    let object = as_object("font", value, problems)?;
    let mut clean = Map::new();
    for (key, value) in object {
        let valid = match key.as_str() {
            "ui" | "mono" => value
                .as_str()
                .is_some_and(|family| !family.trim().is_empty()),
            "medium" | "semibold" => value
                .as_u64()
                .is_some_and(|weight| (100..=900).contains(&weight)),
            _ => {
                unknown("font", key, problems);
                continue;
            }
        };
        if valid {
            clean.insert(key.clone(), value.clone());
        } else {
            invalid("font", key, problems);
        }
    }
    Some(Value::Object(clean))
}

fn clean_mode(mode: &str, value: &Value, problems: &mut Vec<String>) -> Option<Value> {
    let object = as_object(mode, value, problems)?;
    let mut clean = Map::new();
    for (key, value) in object {
        match key.as_str() {
            "colors" => {
                let path = format!("{mode}.colors");
                let Some(colors) = as_object(&path, value, problems) else {
                    continue;
                };
                let mut valid = Map::new();
                for (name, color) in colors {
                    if !is_color_name(name) {
                        invalid(&path, name, problems);
                    } else if color.as_str().and_then(ColorRef::parse).is_some() {
                        valid.insert(name.clone(), color.clone());
                    } else {
                        invalid_color(&path, name, problems);
                    }
                }
                clean.insert(key.clone(), Value::Object(valid));
            }
            "materials" => {
                if let Some(materials) =
                    clean_materials(&format!("{mode}.materials"), value, problems)
                {
                    clean.insert(key.clone(), materials);
                }
            }
            _ => unknown(mode, key, problems),
        }
    }
    Some(Value::Object(clean))
}

fn clean_materials(path: &str, value: &Value, problems: &mut Vec<String>) -> Option<Value> {
    let object = as_object(path, value, problems)?;
    let mut clean = Map::new();
    for (name, value) in object {
        if !MATERIALS.contains(&name.as_str()) {
            unknown(path, name, problems);
            continue;
        }
        let path = format!("{path}.{name}");
        let Some(material) = as_object(&path, value, problems) else {
            continue;
        };
        let mut valid = Map::new();
        for (key, value) in material {
            match key.as_str() {
                "fill" => {
                    let text = value.as_str();
                    if text.and_then(ColorRef::parse).is_some()
                        || text.and_then(GradientSpec::parse).is_some()
                    {
                        valid.insert(key.clone(), value.clone());
                    } else {
                        problems.push(
                            t!("themes.invalid_fill", key = format!("{path}.{key}")).to_string(),
                        );
                    }
                }
                "border" => {
                    if value.as_str().and_then(ColorRef::parse).is_some() {
                        valid.insert(key.clone(), value.clone());
                    } else {
                        invalid_color(&path, key, problems);
                    }
                }
                "texture" | "frame" => {
                    if let Some(clean) = clean_art(&path, key, value, problems) {
                        valid.insert(key.clone(), clean);
                    }
                }
                "shadows" => {
                    let Some(shadows) = value.as_array() else {
                        invalid(&path, key, problems);
                        continue;
                    };
                    let mut kept = Vec::new();
                    for (index, shadow) in shadows.iter().enumerate() {
                        if shadow.as_str().and_then(ShadowSpec::parse).is_some() {
                            kept.push(shadow.clone());
                        } else {
                            invalid(&path, &format!("shadows[{index}]"), problems);
                        }
                    }
                    valid.insert(key.clone(), Value::Array(kept));
                }
                _ => unknown(&path, key, problems),
            }
        }
        clean.insert(name.clone(), Value::Object(valid));
    }
    Some(Value::Object(clean))
}

/// Check a material's `texture` or `frame`: `"none"`, an image file name
/// (textures only) or an object of [`TEXTURE_KEYS`] / [`FRAME_KEYS`].
fn clean_art(path: &str, key: &str, value: &Value, problems: &mut Vec<String>) -> Option<Value> {
    let frame = key == "frame";
    match value {
        Value::String(text) if text.trim() == NONE => return Some(value.clone()),
        Value::String(text) if !frame && is_image_name(text) => return Some(value.clone()),
        Value::Object(_) => {}
        _ => {
            invalid(path, key, problems);
            return None;
        }
    }
    let path = format!("{path}.{key}");
    let object = value.as_object()?;
    let keys: &[&str] = if frame { &FRAME_KEYS } else { &TEXTURE_KEYS };
    let mut clean = Map::new();
    for (name, value) in object {
        if !keys.contains(&name.as_str()) {
            unknown(&path, name, problems);
            continue;
        }
        let valid = match name.as_str() {
            "image" => value.as_str().is_some_and(is_image_name),
            "size" => value
                .as_f64()
                .is_some_and(|size| (1.0..=1000.0).contains(&size)),
            "opacity" => value
                .as_f64()
                .is_some_and(|alpha| (0.0..=1.0).contains(&alpha)),
            "slice" | "width" => edges(value).is_some(),
            _ => value.is_boolean(),
        };
        if valid {
            clean.insert(name.clone(), value.clone());
        } else {
            invalid(&path, name, problems);
        }
    }
    Some(Value::Object(clean))
}

/// A file name such as `paper.png`; no folders.
fn is_image_name(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !text.contains(['/', '\\'])
        && Path::new(text)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// One number for every edge, or `[vertical, horizontal]`, or
/// `[top, right, bottom, left]`, as in CSS.
fn edges(value: &Value) -> Option<[f32; 4]> {
    let number = |value: &Value| {
        value
            .as_f64()
            .filter(|number| (0.0..=1000.0).contains(number))
            .map(|number| number as f32)
    };
    if let Some(all) = number(value) {
        return Some([all; 4]);
    }
    let items = value
        .as_array()?
        .iter()
        .map(number)
        .collect::<Option<Vec<_>>>()?;
    match items[..] {
        [all] => Some([all; 4]),
        [vertical, horizontal] => Some([vertical, horizontal, vertical, horizontal]),
        [top, right, bottom, left] => Some([top, right, bottom, left]),
        _ => None,
    }
}

/// Merge `layer` into the chain above it and resolve the tokens. Returns the
/// theme and the problems of `layer` itself.
fn resolve_layers(dir: &Path, layer: Layer) -> (Theme, Vec<String>) {
    let mut problems = layer.problems.clone();
    let mut seen = BTreeSet::from([layer.id.clone()]);
    let mut chain = vec![layer];
    loop {
        let current = chain.last().expect("theme chain");
        let next = match &current.extends {
            Some(base) => base.clone(),
            None if current.source == ThemeSource::Builtin && current.id == STANDARD => break,
            None => STANDARD.to_string(),
        };
        let next = if seen.contains(&next) || chain.len() >= MAX_CHAIN {
            problems.push(t!("themes.extends_cycle").to_string());
            STANDARD.to_string()
        } else {
            next
        };
        let base = builtin_layer(&next).map(Ok).or_else(|| {
            let source = locate(dir, &next)?;
            Some(read_layer(&next, source))
        });
        let base = match base {
            Some(Ok(base)) => base,
            _ => {
                problems.push(t!("themes.missing_base", id = next).to_string());
                builtin_layer(STANDARD).expect("standard theme")
            }
        };
        seen.insert(base.id.clone());
        chain.push(base);
    }

    // Resolve from the root down so a broken value falls back to what the
    // theme below it had.
    let mut document = Map::new();
    let mut tokens = None;
    let mut icons = BTreeMap::new();
    let mut color_icons = BTreeMap::new();
    let mut images = BTreeMap::new();
    let mut fonts = Vec::<PathBuf>::new();
    let mut base_document = Map::new();
    for (index, layer) in chain.iter().enumerate().rev() {
        if index == 0 {
            base_document = document.clone();
        }
        merge(&mut document, &layer.doc);
        let mut layer_problems = Vec::new();
        tokens = Some(build_tokens(
            &document,
            tokens.as_ref(),
            &mut layer_problems,
        ));
        if index == 0 {
            problems.extend(layer_problems);
        }
        // The nearest theme's icon wins, whichever kind it is; within one
        // package the coloured one does.
        for (name, path) in &layer.icons {
            color_icons.remove(name);
            icons.insert(name.clone(), path.clone());
        }
        for (name, path) in &layer.color_icons {
            icons.remove(name);
            color_icons.insert(name.clone(), path.clone());
        }
        images.extend(layer.images.clone());
        for font in &layer.fonts {
            if !fonts.contains(font) {
                fonts.push(font.clone());
            }
        }
    }
    let tokens = tokens.expect("theme tokens");
    let top = &chain[0];
    report_unused_colors(&top.doc, &base_document, &document, &mut problems);
    report_missing_images(&tokens, &images, &mut problems);
    let mut unique = Vec::new();
    for problem in problems {
        if !unique.contains(&problem) {
            unique.push(problem);
        }
    }
    let theme = Theme {
        id: top.id.clone(),
        name: top.name.clone().unwrap_or_else(|| top.id.clone()),
        source: top.source.clone(),
        extends: top.extends.clone(),
        document,
        tokens,
        icons,
        color_icons,
        images,
        fonts,
    };
    (theme, unique)
}

/// Merge `over` into `base`: objects merge key by key, anything else
/// (numbers, strings, shadow lists) replaces.
fn merge(base: &mut Map<String, Value>, over: &Map<String, Value>) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(Value::Object(base)), Value::Object(over)) => merge(base, over),
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

fn is_color_name(name: &str) -> bool {
    name.starts_with(|first: char| first.is_ascii_lowercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// A colour as written: `#rrggbb(aa)` or the name of another colour.
#[derive(Clone, Debug, PartialEq)]
enum ColorRef {
    Hex(u32),
    Name(String),
}

impl ColorRef {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.starts_with('#') {
            parse_hex(text).map(Self::Hex)
        } else {
            is_color_name(text).then(|| Self::Name(text.to_string()))
        }
    }
}

/// A shadow as written: `[inset] x y [blur [spread]] color`, in pixels.
#[derive(Clone, Debug, PartialEq)]
struct ShadowSpec {
    inset: bool,
    numbers: [f32; 4],
    color: ColorRef,
}

/// A gradient as written: `linear-gradient([angle]deg, colour [stop%],
/// colour [stop%])`. Stops default to 0% and 100%, the angle to 180deg.
#[derive(Clone, Debug, PartialEq)]
struct GradientSpec {
    angle: f32,
    stops: [(ColorRef, f32); 2],
}

impl GradientSpec {
    fn parse(text: &str) -> Option<Self> {
        let inner = text
            .trim()
            .strip_prefix("linear-gradient(")?
            .strip_suffix(')')?;
        let mut parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
        let mut angle = 180.0;
        if let Some(degrees) = parts.first().and_then(|part| part.strip_suffix("deg")) {
            angle = degrees
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|a| a.is_finite())?;
            parts.remove(0);
        }
        let [from, to] = parts[..] else {
            return None;
        };
        let stop = |text: &str, default: f32| {
            let mut words = text.split_whitespace();
            let color = ColorRef::parse(words.next()?)?;
            let at = match words.next() {
                None => default,
                Some(word) => {
                    let percent = word.strip_suffix('%')?.parse::<f32>().ok()?;
                    (0.0..=100.0)
                        .contains(&percent)
                        .then_some(percent / 100.0)?
                }
            };
            words.next().is_none().then_some((color, at))
        };
        Some(Self {
            angle: angle.rem_euclid(360.0),
            stops: [stop(from, 0.0)?, stop(to, 1.0)?],
        })
    }
}

impl ShadowSpec {
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split_whitespace().collect::<Vec<_>>();
        let inset = parts.first() == Some(&"inset");
        if inset {
            parts.remove(0);
        }
        let color = ColorRef::parse(parts.pop()?)?;
        if !(2..=4).contains(&parts.len()) {
            return None;
        }
        let mut numbers = [0.0; 4];
        for (slot, part) in numbers.iter_mut().zip(&parts) {
            let number = part
                .strip_suffix("px")
                .unwrap_or(part)
                .parse::<f32>()
                .ok()?;
            if !number.is_finite() {
                return None;
            }
            *slot = number;
        }
        (numbers[2] >= 0.0).then_some(Self {
            inset,
            numbers,
            color,
        })
    }
}

fn build_tokens(
    document: &Map<String, Value>,
    fallback: Option<&Tokens>,
    problems: &mut Vec<String>,
) -> Tokens {
    let group = |key: &str| document.get(key).and_then(Value::as_object);
    let font = group("font");
    let font_str = |key: &str, fallback: Option<&String>| {
        font.and_then(|font| font.get(key))
            .and_then(Value::as_str)
            .map(|family| family.trim().to_string())
            .or_else(|| fallback.cloned())
            .unwrap_or_default()
    };
    let font_weight = |key: &str, fallback: Option<u16>| {
        font.and_then(|font| font.get(key))
            .and_then(Value::as_u64)
            .map(|weight| weight as u16)
            .or(fallback)
            .unwrap_or(400)
    };
    let numbers = |key: &str, keys: &[&'static str], fallback: Option<&Numbers>| {
        keys.iter()
            .map(|name| {
                let value = group(key)
                    .and_then(|group| group.get(*name))
                    .and_then(Value::as_f64)
                    .map(|value| value as f32)
                    .or_else(|| fallback.and_then(|numbers| numbers.get(name).copied()))
                    .unwrap_or_default();
                (*name, value)
            })
            .collect::<Numbers>()
    };
    let shape = numbers("shape", &SHAPE_KEYS, fallback.map(|tokens| &tokens.shape));
    let border = shape.get("border").copied().unwrap_or_default();
    Tokens {
        font: Font {
            ui: font_str("ui", fallback.map(|tokens| &tokens.font.ui)),
            mono: font_str("mono", fallback.map(|tokens| &tokens.font.mono)),
            medium: font_weight("medium", fallback.map(|tokens| tokens.font.medium)),
            semibold: font_weight("semibold", fallback.map(|tokens| tokens.font.semibold)),
        },
        text: numbers("text", &TEXT_KEYS, fallback.map(|tokens| &tokens.text)),
        size: numbers("size", &SIZE_KEYS, fallback.map(|tokens| &tokens.size)),
        light: build_mode(
            document,
            "light",
            border,
            fallback.map(|tokens| &tokens.light),
            problems,
        ),
        dark: build_mode(
            document,
            "dark",
            border,
            fallback.map(|tokens| &tokens.dark),
            problems,
        ),
        shape,
    }
}

fn build_mode(
    document: &Map<String, Value>,
    mode: &str,
    border: f32,
    fallback: Option<&Mode>,
    problems: &mut Vec<String>,
) -> Mode {
    let mode_doc = document.get(mode).and_then(Value::as_object);
    let raw = mode_doc
        .and_then(|mode| mode.get("colors"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut colors = BTreeMap::new();
    for name in raw.keys() {
        let color = resolve_color(&format!("{mode}.colors.{name}"), name, &raw, problems)
            .or_else(|| fallback.and_then(|mode| mode.colors.get(name)).copied());
        if let Some(color) = color {
            colors.insert(name.clone(), color);
        }
    }
    for key in COLOR_KEYS {
        if !colors.contains_key(key) {
            let color = fallback
                .and_then(|mode| mode.colors.get(key))
                .copied()
                .unwrap_or_default();
            colors.insert(key.to_string(), color);
        }
    }
    for (key, base) in OPTIONAL_COLOR_KEYS {
        if !colors.contains_key(key)
            && let Some(color) = base.and_then(|base| colors.get(base)).copied()
        {
            colors.insert(key.to_string(), color);
        }
    }

    let materials_of = |doc: Option<&Map<String, Value>>, name: &str| {
        doc.and_then(|doc| doc.get("materials"))
            .and_then(Value::as_object)
            .and_then(|materials| materials.get(name))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let mut materials = BTreeMap::new();
    for name in MATERIALS {
        let mut spec = materials_of(Some(document), name);
        merge(&mut spec, &materials_of(mode_doc, name));
        let path = format!("materials.{name}");
        let color = |key: &str, problems: &mut Vec<String>| {
            let text = spec.get(key)?.as_str()?;
            lookup(&format!("{path}.{key}"), text, &colors, problems)
        };
        let fill = spec
            .get("fill")
            .and_then(Value::as_str)
            .and_then(|text| paint(&format!("{path}.fill"), text, &colors, problems));
        let border_color = color("border", problems);
        let inner_offset = if BORDERED_MATERIALS.contains(&name) {
            border
        } else {
            0.0
        };
        let shadows = spec
            .get("shadows")
            .and_then(Value::as_array)
            .map(|shadows| {
                shadows
                    .iter()
                    .filter_map(|shadow| ShadowSpec::parse(shadow.as_str()?))
                    .filter_map(|shadow| {
                        let color = match &shadow.color {
                            ColorRef::Hex(color) => *color,
                            ColorRef::Name(name) => {
                                lookup(&format!("{path}.shadows"), name, &colors, problems)?
                            }
                        };
                        let [x, y, blur, spread] = shadow.numbers;
                        Some(Shadow {
                            x,
                            y,
                            blur,
                            // GPUI paints inset shadows under the border, CSS
                            // inside it; shrinking the hole by the border width
                            // gives the CSS result.
                            spread: if shadow.inset {
                                spread + inner_offset
                            } else {
                                spread
                            },
                            color,
                            inset: shadow.inset,
                        })
                    })
                    .collect()
            });
        materials.insert(
            name,
            MaterialStyle {
                fill,
                border: border_color,
                shadows,
                texture: art(&spec, "texture", &path, texture, problems),
                frame: art(&spec, "frame", &path, frame, problems),
            },
        );
    }
    Mode {
        dark: mode == "dark",
        colors,
        materials,
    }
}

fn paint(
    path: &str,
    text: &str,
    colors: &BTreeMap<String, u32>,
    problems: &mut Vec<String>,
) -> Option<Paint> {
    let Some(gradient) = GradientSpec::parse(text) else {
        return lookup(path, text, colors, problems).map(Paint::Solid);
    };
    let mut stop = |(color, at): &(ColorRef, f32)| {
        let color = match color {
            ColorRef::Hex(color) => Some(*color),
            ColorRef::Name(name) => lookup(path, name, colors, problems),
        };
        Some((color?, *at))
    };
    let from = stop(&gradient.stops[0]);
    let to = stop(&gradient.stops[1]);
    Some(Paint::Gradient {
        angle: gradient.angle,
        from: from?,
        to: to?,
    })
}

/// The merged `texture` or `frame` of a material; objects missing a
/// required key are reported and skipped.
fn art<T>(
    spec: &Map<String, Value>,
    key: &str,
    path: &str,
    read: fn(&Value) -> Option<T>,
    problems: &mut Vec<String>,
) -> Option<T> {
    let value = spec.get(key)?;
    let art = read(value);
    if art.is_none()
        && let Some(object) = value.as_object()
    {
        let required: &[&str] = if key == "frame" {
            &["image", "slice"]
        } else {
            &["image"]
        };
        if required.iter().any(|name| !object.contains_key(*name)) {
            let key = format!("{path}.{key}");
            problems.push(
                t!(
                    "themes.incomplete_art",
                    key = key,
                    keys = required.join(", ")
                )
                .to_string(),
            );
        }
    }
    art
}

fn texture(value: &Value) -> Option<Texture> {
    if let Some(text) = value.as_str() {
        return (text.trim() != NONE).then(|| Texture {
            image: text.trim().to_string(),
            size: None,
            opacity: 1.0,
            pixelated: false,
        });
    }
    let object = value.as_object()?;
    Some(Texture {
        image: object.get("image")?.as_str()?.trim().to_string(),
        size: object
            .get("size")
            .and_then(Value::as_f64)
            .map(|size| size as f32),
        opacity: object.get("opacity").and_then(Value::as_f64).unwrap_or(1.0) as f32,
        pixelated: object
            .get("pixelated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn frame(value: &Value) -> Option<Frame> {
    let object = value.as_object()?;
    let flag = |key: &str| object.get(key).and_then(Value::as_bool).unwrap_or(false);
    let slice = edges(object.get("slice")?)?;
    Some(Frame {
        image: object.get("image")?.as_str()?.trim().to_string(),
        slice,
        width: object.get("width").and_then(edges).unwrap_or(slice),
        repeat: flag("repeat"),
        fill: flag("fill"),
        pixelated: flag("pixelated"),
    })
}

/// Report textures and frames whose image isn't in any package of the chain.
fn report_missing_images(
    tokens: &Tokens,
    images: &BTreeMap<String, PathBuf>,
    problems: &mut Vec<String>,
) {
    for mode in [&tokens.light, &tokens.dark] {
        for (name, style) in &mode.materials {
            let art = [
                (
                    "texture",
                    style.texture.as_ref().map(|texture| &texture.image),
                ),
                ("frame", style.frame.as_ref().map(|frame| &frame.image)),
            ];
            for (key, image) in art {
                if let Some(image) = image
                    && !images.contains_key(image)
                {
                    problems.push(
                        t!(
                            "themes.unknown_image",
                            key = format!("materials.{name}.{key}"),
                            image = image
                        )
                        .to_string(),
                    );
                }
            }
        }
    }
}

fn lookup(
    path: &str,
    text: &str,
    colors: &BTreeMap<String, u32>,
    problems: &mut Vec<String>,
) -> Option<u32> {
    match ColorRef::parse(text)? {
        ColorRef::Hex(color) => Some(color),
        ColorRef::Name(name) => {
            let color = colors.get(&name).copied();
            if color.is_none() {
                problems.push(t!("themes.unknown_color", key = path, color = name).to_string());
            }
            color
        }
    }
}

/// Follow colour names in `raw` until one ends in a hex value.
fn resolve_color(
    path: &str,
    name: &str,
    raw: &Map<String, Value>,
    problems: &mut Vec<String>,
) -> Option<u32> {
    let mut current = name.to_string();
    for _ in 0..MAX_CHAIN {
        let Some(text) = raw.get(&current).and_then(Value::as_str) else {
            problems.push(t!("themes.unknown_color", key = path, color = current).to_string());
            return None;
        };
        match ColorRef::parse(text)? {
            ColorRef::Hex(color) => return Some(color),
            ColorRef::Name(next) => current = next,
        }
    }
    problems.push(t!("themes.unknown_color", key = path, color = current).to_string());
    None
}

/// Report named colours the theme adds but nothing uses: likely typos of a
/// known key. Names its base themes define are never typos.
fn report_unused_colors(
    own: &Map<String, Value>,
    base: &Map<String, Value>,
    document: &Map<String, Value>,
    problems: &mut Vec<String>,
) {
    let mut used = BTreeSet::new();
    for key in ["materials", "light", "dark"] {
        if let Some(value) = document.get(key) {
            collect_names(value, &mut used);
        }
    }
    for mode in ["light", "dark"] {
        let Some(colors) = own
            .get(mode)
            .and_then(|mode| mode.get("colors"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for name in colors.keys() {
            let known = COLOR_KEYS.contains(&name.as_str())
                || OPTIONAL_COLOR_KEYS.iter().any(|(key, _)| key == name)
                || base
                    .get(mode)
                    .and_then(|mode| mode.get("colors"))
                    .is_some_and(|colors| colors.get(name).is_some());
            if !known && !used.contains(name) {
                problems.push(
                    t!("themes.unused_color", key = format!("{mode}.colors.{name}")).to_string(),
                );
            }
        }
    }
}

/// Colour names referenced by colour values, material colours and shadows.
fn collect_names(value: &Value, used: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if let Some(gradient) = GradientSpec::parse(text) {
                for (color, _) in gradient.stops {
                    if let ColorRef::Name(name) = color {
                        used.insert(name);
                    }
                }
                return;
            }
            let last = text.split_whitespace().last().unwrap_or_default();
            if let Some(ColorRef::Name(name)) = ColorRef::parse(last) {
                used.insert(name);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_names(item, used)),
        Value::Object(map) => map.values().for_each(|item| collect_names(item, used)),
        _ => {}
    }
}

fn parse_hex(text: &str) -> Option<u32> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some((value << 8) | 0xff),
        8 => Some(value),
        _ => None,
    }
}

/// `#rrggbb` or `#rrggbbaa` as `0xRRGGBBAA`.
pub fn parse_color(value: &Value) -> Option<u32> {
    parse_hex(value.as_str()?)
}

pub fn format_color(color: u32) -> String {
    if color & 0xff == 0xff {
        format!("#{:06x}", color >> 8)
    } else {
        format!("#{color:08x}")
    }
}

/// A complete `theme.json` for `theme`: every key of its chain written out.
pub fn to_json(theme: &Theme) -> Value {
    let mut root = Map::new();
    root.insert("name".into(), theme.name.clone().into());
    if let Some(base) = &theme.extends {
        root.insert("extends".into(), base.clone().into());
    }
    root.extend(theme.document.clone());
    Value::Object(root)
}

/// [`to_json`] as pretty text, keys in the documented order.
pub fn to_json_text(theme: &Theme) -> String {
    let mut text = String::new();
    write_value(&mut text, "", &to_json(theme), 0);
    text.push('\n');
    text
}

fn key_order(parent: &str) -> Vec<&'static str> {
    match parent {
        "" => ROOT_KEYS.to_vec(),
        "font" => FONT_KEYS.to_vec(),
        "text" => TEXT_KEYS.to_vec(),
        "shape" => SHAPE_KEYS.to_vec(),
        "size" => SIZE_KEYS.to_vec(),
        "materials" => MATERIALS.to_vec(),
        "light" | "dark" => MODE_KEYS.to_vec(),
        "colors" => COLOR_KEYS
            .into_iter()
            .chain(OPTIONAL_COLOR_KEYS.into_iter().map(|(key, _)| key))
            .collect(),
        name if MATERIALS.contains(&name) => MATERIAL_KEYS.to_vec(),
        "texture" => TEXTURE_KEYS.to_vec(),
        "frame" => FRAME_KEYS.to_vec(),
        _ => vec![],
    }
}

fn write_value(out: &mut String, parent: &str, value: &Value, depth: usize) {
    let indent = |depth: usize| "  ".repeat(depth);
    match value {
        Value::Object(map) if !map.is_empty() => {
            let order = key_order(parent);
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort_by_key(|key| {
                (
                    order
                        .iter()
                        .position(|known| known == key)
                        .unwrap_or(usize::MAX),
                    (*key).clone(),
                )
            });
            out.push_str("{\n");
            for (index, key) in keys.iter().enumerate() {
                let _ = write!(out, "{}{}: ", indent(depth + 1), Value::from(key.as_str()));
                write_value(out, key, &map[key.as_str()], depth + 1);
                out.push_str(if index + 1 < keys.len() { ",\n" } else { "\n" });
            }
            let _ = write!(out, "{}}}", indent(depth));
        }
        Value::Array(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                out.push_str(&indent(depth + 1));
                write_value(out, "", item, depth + 1);
                out.push_str(if index + 1 < items.len() { ",\n" } else { "\n" });
            }
            let _ = write!(out, "{}]", indent(depth));
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Create the theme package `<dir>/<id>/` holding every key of `from`, with
/// empty `icons/` and `fonts/` folders.
pub fn create_in(dir: &Path, id: &str, name: Option<&str>, from: &Theme) -> Result<PathBuf> {
    check_id(id)?;
    if locate(dir, id).is_some() {
        crate::user_bail!("errors.theme_exists", id = id);
    }
    let package = dir.join(id);
    let mut theme = from.clone();
    theme.name = name.map(str::to_string).unwrap_or_else(|| id.to_string());
    theme.extends = Some(from.id.clone());
    fs::create_dir_all(package.join(ICONS_DIR))?;
    fs::create_dir_all(package.join(FONTS_DIR))?;
    fs::create_dir_all(package.join(IMAGES_DIR))?;
    fs::write(package.join(MANIFEST), to_json_text(&theme))?;
    Ok(package)
}

/// Copy a theme file or package folder into `dir`, named after it unless
/// `id` is given.
pub fn add_in(dir: &Path, source: &Path, id: Option<&str>, force: bool) -> Result<ThemeFile> {
    let package = source.is_dir();
    let id = match id {
        Some(id) => id.to_string(),
        None => if package {
            source.file_name()
        } else {
            source.file_stem()
        }
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string(),
    };
    check_id(&id)?;
    let manifest = if package {
        source.join(MANIFEST)
    } else {
        source.to_path_buf()
    };
    let text = fs::read_to_string(&manifest)
        .with_context(|| t!("errors.theme_read_failed", path = manifest.display()).to_string())?;
    if let Err(error) = parse_layer(&id, &text, ThemeSource::File(manifest.clone())) {
        crate::user_bail!("errors.theme_invalid", id = id, error = error);
    }
    if locate(dir, &id).is_some() {
        if !force {
            crate::user_bail!("errors.theme_exists", id = id);
        }
        remove_in(dir, &id)?;
    }
    fs::create_dir_all(dir)?;
    if package {
        copy_dir(source, &dir.join(&id))?;
    } else {
        fs::write(dir.join(format!("{id}.json")), text)?;
    }
    load_in(dir, &id).ok_or_else(|| crate::user_error!("errors.theme_not_found", id = id).into())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn remove_in(dir: &Path, id: &str) -> Result<()> {
    check_id(id)?;
    match locate(dir, id) {
        Some(ThemeSource::Package(path)) => fs::remove_dir_all(path)?,
        Some(ThemeSource::File(path)) => fs::remove_file(path)?,
        _ => crate::user_bail!("errors.theme_not_found", id = id),
    }
    // A package hides a file of the same id; remove that too so the id is
    // really gone.
    let file = dir.join(format!("{id}.json"));
    if file.is_file() {
        fs::remove_file(file)?;
    }
    Ok(())
}

fn check_id(id: &str) -> Result<()> {
    if BUILTIN_THEMES.contains(&id) {
        crate::user_bail!("errors.theme_builtin", id = id);
    }
    if !is_file_id(id) {
        crate::user_bail!("errors.theme_invalid_id", id = id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn check(dir: &Path, id: &str) -> (Theme, Vec<String>) {
        let file = load_in(dir, id).unwrap();
        (file.theme.unwrap(), file.problems)
    }

    #[test]
    fn built_in_themes_define_every_token_without_problems() {
        for id in BUILTIN_THEMES {
            let layer = builtin_layer(id).unwrap();
            assert!(layer.problems.is_empty(), "{id}: {:?}", layer.problems);
            let (theme, problems) = resolve_layers(Path::new(""), layer);
            assert!(problems.is_empty(), "{id}: {problems:?}");
            for mode in ["light", "dark"] {
                let raw = theme.document[mode]["colors"].as_object().unwrap();
                for key in COLOR_KEYS {
                    assert!(raw.contains_key(key), "{id} {mode} {key}");
                }
            }
            for (group, keys) in [
                ("shape", &SHAPE_KEYS[..]),
                ("text", &TEXT_KEYS[..]),
                ("size", &SIZE_KEYS[..]),
                ("font", &FONT_KEYS[..]),
            ] {
                for key in keys {
                    assert!(theme.document[group].get(*key).is_some(), "{group}.{key}");
                }
            }
        }
    }

    #[test]
    fn solid_glass_keeps_its_colours_and_finish() {
        let glass = builtin(SOLID_GLASS).unwrap().tokens;
        let light = &glass.light;
        assert_eq!(light.colors["surface"], 0xeceef2ff);
        assert_eq!(light.colors["dialog_base"], light.colors["elevated"]);
        assert_eq!(light.colors["dialog_surface"], 0xf3f4f7ff);
        assert_eq!(glass.dark.colors["switch_thumb"], 0xf5f5f5ff);
        let pane = &light.materials["pane"];
        assert_eq!(pane.border, Some(0x3b42531a));
        assert_eq!(
            pane.shadows.as_ref().unwrap()[0],
            Shadow {
                x: 0.,
                y: 1.,
                blur: 0.,
                spread: 1.,
                color: 0xffffff73,
                inset: true,
            }
        );
        // Unbordered materials keep their inset shadows as written.
        let state = &light.materials["state"].shadows.as_ref().unwrap()[1];
        assert_eq!((state.spread, state.inset), (1., true));
        assert_eq!(glass.shape["radius_md"], 8.);

        let standard = builtin(STANDARD).unwrap().tokens;
        assert_eq!(standard.light.colors["window"], 0xf5f5f5ff);
        assert_eq!(standard.light.colors["dialog_surface"], 0xffffffff);
        assert!(!standard.light.colors.contains_key("switch_track"));
        assert_eq!(standard.dark.materials["pane"], MaterialStyle::default());
        assert_eq!(
            standard.light.materials["popover"]
                .shadows
                .as_ref()
                .unwrap()[0]
                .y,
            10.
        );
        assert_eq!(standard.font.ui, ".SystemUIFont");
    }

    #[test]
    fn parses_hex_colours_and_shadows() {
        assert_eq!(parse_color(&"#ff6600".into()), Some(0xff6600ff));
        assert_eq!(parse_color(&"#FF660080".into()), Some(0xff660080));
        for bad in ["ff6600", "#f60", "#gg6600", "#+f6600"] {
            assert_eq!(parse_color(&bad.into()), None, "{bad}");
        }
        assert_eq!(format_color(0xff6600ff), "#ff6600");
        assert_eq!(format_color(0xff660080), "#ff660080");

        let shadow = ShadowSpec::parse("inset 0 1px 2 -3 rim").unwrap();
        assert!(shadow.inset);
        assert_eq!(shadow.numbers, [0., 1., 2., -3.]);
        assert_eq!(shadow.color, ColorRef::Name("rim".into()));
        assert_eq!(
            ShadowSpec::parse("2 2 #000000").unwrap().numbers,
            [2., 2., 0., 0.]
        );
        for bad in ["0 1", "1 rim", "0 0 -1 0 rim", "0 0 0 0 0 rim", "0 0 Rim"] {
            assert!(ShadowSpec::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn overrides_tokens_and_reports_what_it_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "ocean.json",
            r##"{
              "extends": "solid_glass", "extra": 1,
              "shape": {"radius_md": 4, "radius_huge": 2, "border": -1},
              "font": {"ui": "Menlo", "medium": 50},
              "materials": {"pane": {"shadows": ["0 2 4 0 glow", "bad"]}, "bogus": {}},
              "light": {"colors": {"accent": "#ff6600", "focus": "accent", "glow": "#00ff0080",
                                   "typo": "#000000", "text": "bad!", "rim": "nope"}}
            }"##,
        );
        let (theme, problems) = check(dir.path(), "ocean");
        let base = builtin(SOLID_GLASS).unwrap().tokens;
        let tokens = &theme.tokens;
        assert_eq!(theme.name, "ocean");
        assert_eq!(tokens.shape["radius_md"], 4.);
        assert_eq!(tokens.shape["border"], 1.);
        assert_eq!(tokens.font.ui, "Menlo");
        assert_eq!(tokens.font.medium, 500);
        assert_eq!(tokens.light.colors["focus"], 0xff6600ff);
        assert_eq!(tokens.light.colors["text"], base.light.colors["text"]);
        assert_eq!(tokens.light.colors["rim"], base.light.colors["rim"]);
        let pane = tokens.light.materials["pane"].clone();
        assert_eq!(pane.border, base.light.materials["pane"].border);
        assert_eq!(pane.shadows.unwrap()[0].color, 0x00ff0080);
        // The dark mode has no `glow`, so its shadow is dropped and reported.
        let mut dark = base.dark.clone();
        dark.materials.get_mut("pane").unwrap().shadows = Some(vec![]);
        assert_eq!(tokens.dark, dark);
        // extra, radius_huge, border, medium, shadows[1], bogus, text,
        // rim -> nope, typo unused, glow missing in dark.
        assert_eq!(problems.len(), 10, "{problems:#?}");
    }

    #[test]
    fn follows_extends_chains_and_survives_loops() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "base.json",
            r##"{"extends":"solid_glass","shape":{"radius_md":3}}"##,
        );
        write(
            dir.path(),
            "child.json",
            r##"{"extends":"base","light":{"colors":{"accent":"#123456"}}}"##,
        );
        write(dir.path(), "a.json", r#"{"extends":"b"}"#);
        write(dir.path(), "b.json", r#"{"extends":"a"}"#);
        write(dir.path(), "orphan.json", r#"{"extends":"gone"}"#);

        let (child, problems) = check(dir.path(), "child");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(child.tokens.shape["radius_md"], 3.);
        assert_eq!(child.tokens.light.colors["accent"], 0x123456ff);
        let glass = builtin(SOLID_GLASS).unwrap().tokens;
        assert_eq!(child.tokens.light.materials, glass.light.materials);

        let standard = builtin(STANDARD).unwrap().tokens;
        let (looped, problems) = check(dir.path(), "a");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(looped.tokens, standard);
        let (orphan, problems) = check(dir.path(), "orphan");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(orphan.tokens, standard);
    }

    #[test]
    fn packages_bring_icons_and_fonts_down_the_chain() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "pixel/theme.json", r#"{"name":"Pixel"}"#);
        write(dir.path(), "pixel/icons/folder.svg", "<svg/>");
        write(dir.path(), "pixel/icons/trash.svg", "<svg/>");
        write(dir.path(), "pixel/icons/nonsense.svg", "<svg/>");
        write(dir.path(), "pixel/fonts/Pixel.ttf", "");
        write(dir.path(), "pixel/fonts/readme.txt", "");
        write(dir.path(), "pixel.json", r#"{"name":"Hidden"}"#);
        write(dir.path(), "night/theme.json", r#"{"extends":"pixel"}"#);
        write(dir.path(), "night/icons/trash.svg", "<svg/>");

        let (pixel, problems) = check(dir.path(), "pixel");
        assert_eq!(pixel.name, "Pixel");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(pixel.fonts, [dir.path().join("pixel/fonts/Pixel.ttf")]);
        let (night, problems) = check(dir.path(), "night");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            night.icons["folder"],
            dir.path().join("pixel/icons/folder.svg")
        );
        assert_eq!(
            night.icons["trash"],
            dir.path().join("night/icons/trash.svg")
        );
        assert_eq!(night.fonts, pixel.fonts);
    }

    #[test]
    fn materials_take_gradients_textures_and_frames() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "desk/theme.json",
            r##"{
              "materials": {
                "window": { "texture": { "image": "grain.png", "size": 64, "opacity": 0.5 } },
                "pane": {
                  "fill": "linear-gradient(90deg, top 10%, #000000)",
                  "frame": { "image": "bevel.png", "slice": [4, 6], "repeat": true, "pixelated": true }
                },
                "control": { "texture": "paper.png", "frame": { "image": "bevel.png" } },
                "field": { "fill": "linear-gradient(top, bottom)" }
              },
              "light": { "colors": { "top": "#ffffff" } },
              "dark": { "colors": { "top": "#101010" } }
            }"##,
        );
        write(dir.path(), "desk/images/grain.png", "");
        write(dir.path(), "desk/images/bevel.png", "");
        write(dir.path(), "desk/icons/color/folder.svg", "<svg/>");
        write(dir.path(), "desk/icons/folder.svg", "<svg/>");
        write(dir.path(), "desk/icons/color/nonsense.svg", "<svg/>");
        write(dir.path(), "plain/theme.json", r#"{"extends":"desk"}"#);
        write(dir.path(), "plain/icons/folder.svg", "<svg/>");

        let (desk, problems) = check(dir.path(), "desk");
        let light = &desk.tokens.light.materials;
        assert_eq!(
            light["pane"].fill,
            Some(Paint::Gradient {
                angle: 90.,
                from: (0xffffffff, 0.1),
                to: (0x000000ff, 1.),
            })
        );
        assert_eq!(
            desk.tokens.dark.materials["pane"].fill,
            Some(Paint::Gradient {
                angle: 90.,
                from: (0x101010ff, 0.1),
                to: (0x000000ff, 1.),
            })
        );
        let frame = light["pane"].frame.as_ref().unwrap();
        assert_eq!(frame.slice, [4., 6., 4., 6.]);
        assert_eq!(frame.width, frame.slice);
        assert!(frame.repeat && frame.pixelated && !frame.fill);
        let texture = light["window"].texture.as_ref().unwrap();
        assert_eq!((texture.size, texture.opacity), (Some(64.), 0.5));
        assert_eq!(
            light["control"].texture.as_ref().unwrap().image,
            "paper.png"
        );
        assert!(light["field"].fill.is_none());
        assert_eq!(problems.len(), 4, "{problems:?}");
        for key in [
            "materials.control.frame",
            "materials.field.fill",
            "images/paper.png",
            "color/nonsense",
        ] {
            assert!(
                problems.iter().any(|problem| problem.contains(key)),
                "{key}: {problems:?}"
            );
        }
        assert_eq!(
            desk.color_icons["folder"],
            dir.path().join("desk/icons/color/folder.svg")
        );
        assert!(!desk.icons.contains_key("folder"));
        assert!(desk.images.contains_key("grain.png"));

        // The nearest theme's icon wins over an inherited coloured one.
        let (plain, _) = check(dir.path(), "plain");
        assert!(plain.color_icons.is_empty());
        assert_eq!(plain.images, desk.images);
        let removed = parse_layer(
            "x",
            r#"{"materials":{"window":{"texture":"none"}}}"#,
            ThemeSource::Builtin,
        )
        .unwrap();
        assert!(removed.problems.is_empty());
    }

    #[test]
    fn missing_or_broken_themes_fall_back_to_standard() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "broken.json", "{");
        write(dir.path(), "bad/theme.json", "[]");
        for id in ["broken", "bad", "missing", "../x"] {
            assert!(find_in(dir.path(), id).is_err(), "{id}");
            assert_eq!(resolve_in(dir.path(), id).id, STANDARD, "{id}");
        }
    }

    #[test]
    fn lists_user_themes_by_name_and_keeps_broken_ones() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "b.json", r#"{"name":"Zest"}"#);
        write(dir.path(), "a.json", r#"{"name":"apple"}"#);
        write(dir.path(), "plain.json", "{}");
        write(dir.path(), "broken.json", "[");
        write(dir.path(), "solid_glass.json", "{}");
        write(dir.path(), "notes.txt", "{}");
        write(dir.path(), "kit/theme.json", r#"{"name":"Kit"}"#);
        write(dir.path(), "empty/readme.txt", "");
        let files = list_files_in(dir.path())
            .into_iter()
            .map(|file| (file.id, file.theme.is_ok()))
            .collect::<Vec<_>>();
        assert_eq!(
            files,
            [
                ("a".into(), true),
                ("broken".into(), false),
                ("kit".into(), true),
                ("plain".into(), true),
                ("b".into(), true),
            ]
        );
        let ids = list_in(dir.path())
            .into_iter()
            .map(|theme| theme.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, [STANDARD, SOLID_GLASS, "a", "kit", "plain", "b"]);
    }

    #[test]
    fn created_packages_round_trip_to_the_same_tokens() {
        let dir = tempfile::tempdir().unwrap();
        for from in BUILTIN_THEMES {
            let base = builtin(from).unwrap();
            let id = format!("mine-{from}");
            let path = create_in(dir.path(), &id, Some("Mine"), &base).unwrap();
            assert!(
                [ICONS_DIR, FONTS_DIR, IMAGES_DIR]
                    .iter()
                    .all(|d| path.join(d).is_dir())
            );
            let (theme, problems) = check(dir.path(), &id);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(theme.name, "Mine");
            assert_eq!(theme.extends.as_deref(), Some(from));
            assert_eq!(theme.tokens, base.tokens);
            assert!(create_in(dir.path(), &id, None, &base).is_err());
        }
        let base = builtin(STANDARD).unwrap();
        assert!(create_in(dir.path(), STANDARD, None, &base).is_err());
        assert!(create_in(dir.path(), "../x", None, &base).is_err());
        let text = to_json_text(&builtin(SOLID_GLASS).unwrap());
        let order = [
            "\"name\"",
            "\"extends\"",
            "\"font\"",
            "\"materials\"",
            "\"light\"",
        ];
        let positions = order.map(|key| text.find(key).unwrap());
        assert!(positions.is_sorted(), "{text}");
    }

    #[test]
    fn adds_and_removes_theme_files_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        write(
            source.path(),
            "ocean.json",
            r##"{"light":{"colors":{"accent":"#ff6600"}}}"##,
        );
        write(source.path(), "broken.json", "{");
        write(source.path(), "pixel/theme.json", r#"{"name":"Pixel"}"#);
        write(source.path(), "pixel/icons/x.svg", "<svg/>");
        write(source.path(), "nothing/readme.txt", "");
        let ocean = source.path().join("ocean.json");
        assert_eq!(add_in(dir.path(), &ocean, None, false).unwrap().id, "ocean");
        assert!(add_in(dir.path(), &ocean, None, false).is_err());
        assert!(add_in(dir.path(), &ocean, None, true).is_ok());
        assert_eq!(
            add_in(dir.path(), &ocean, Some("sea"), false).unwrap().id,
            "sea"
        );
        assert!(add_in(dir.path(), &source.path().join("broken.json"), None, false).is_err());
        assert!(!dir.path().join("broken.json").exists());
        assert!(add_in(dir.path(), &source.path().join("nothing"), None, false).is_err());

        let pixel = add_in(dir.path(), &source.path().join("pixel"), None, false).unwrap();
        assert!(pixel.theme.unwrap().icons.contains_key("x"));
        // A package replaces a file of the same id when forced.
        assert!(add_in(dir.path(), &source.path().join("pixel"), Some("sea"), false).is_err());
        add_in(dir.path(), &source.path().join("pixel"), Some("sea"), true).unwrap();
        assert!(!dir.path().join("sea.json").exists());
        assert!(dir.path().join("sea").join(MANIFEST).is_file());

        remove_in(dir.path(), "ocean").unwrap();
        remove_in(dir.path(), "pixel").unwrap();
        assert!(find_in(dir.path(), "ocean").is_err());
        assert!(!dir.path().join("pixel").exists());
        assert!(remove_in(dir.path(), "ocean").is_err());
        assert!(remove_in(dir.path(), STANDARD).is_err());
    }
}
