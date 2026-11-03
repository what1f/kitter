use std::{
    borrow::Cow,
    collections::BTreeSet,
    path::PathBuf,
    sync::{LazyLock, Mutex, RwLock},
};

use gpui::{App, Background, BoxShadow, FontWeight, Hsla, Rgba, SharedString, Styled, Window};
use gpui_component::{Theme as ComponentTheme, ThemeMode as ComponentThemeMode};

use super::{
    surface_art::{FrameArt, TextureArt},
    theme_file::{self, ActiveTheme},
};

pub(super) const DESCRIPTION_MAX_WIDTH: f32 = 700.;
pub(super) const DESCRIPTION_COLLAPSED_LINES: usize = 3;
pub(super) const SETTINGS_CONTENT_MAX_WIDTH: f32 = 768.;
pub(super) const CONTEXT_TOKEN_WARNING_THRESHOLD: usize = 2_000;
pub(super) const CONTEXT_TOKEN_DANGER_THRESHOLD: usize = 5_000;
pub(super) const CONTEXT_COUNT_WARNING_THRESHOLD: usize = 20;
pub(super) const CONTEXT_COUNT_DANGER_THRESHOLD: usize = 50;

/// The theme applied by the last `apply_component_theme` call; the standard
/// theme until one is applied. Every token below reads from it.
static ACTIVE: LazyLock<RwLock<ActiveTheme>> =
    LazyLock::new(|| RwLock::new(theme_file::standard()));

fn with_active<T>(read: impl FnOnce(&ActiveTheme) -> T) -> T {
    match ACTIVE.read() {
        Ok(active) => read(&active),
        Err(poisoned) => read(&poisoned.into_inner()),
    }
}

macro_rules! metrics {
    ($($(#[$doc:meta])* $field:ident: $group:ident.$key:literal,)*) => {
        /// Sizes of the active theme, in pixels. See `docs/themes.md`.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub(super) struct Metrics {
            $(pub $field: f32,)*
        }

        impl Metrics {
            pub(super) fn from_tokens(tokens: &crate::themes::Tokens) -> Self {
                Self {
                    $($field: tokens.$group.get($key).copied().unwrap_or_default(),)*
                }
            }
        }

        $(
            $(#[$doc])*
            pub(super) fn $field() -> f32 {
                with_active(|active| active.metrics.$field)
            }
        )*
    };
}

// Design tokens. Components read sizes from here instead of hard-coding
// them, so a theme can change every one.
metrics! {
    /// Badge, chip, menu item, tooltip, small icon tile.
    r_sm: shape."radius_sm",
    /// Button, input, select, tab, list row.
    r_md: shape."radius_md",
    /// Menu, popover, toast, 40px icon tile.
    r_lg: shape."radius_lg",
    /// Card.
    r_xl: shape."radius_xl",
    /// Dialog.
    r_modal: shape."radius_modal",
    /// Avatars, dots and other round shapes.
    r_pill: shape."radius_pill",
    /// Outline of cards, controls, fields and popovers.
    border_w: shape."border",
    /// Lines between rows and sections.
    divider_w: shape."divider",

    fs_caption: text."caption",
    fs_small: text."small",
    fs_body: text."body",
    fs_emph: text."emphasis",
    fs_title: text."title",
    fs_display: text."display",

    /// Toolbars, inline actions, menu items and tabs.
    h_sm: size."control_sm",
    /// Form controls, dialog buttons and sidebar navigation.
    h_md: size."control_md",
    /// Single-line list rows.
    row_1: size."row",
    /// Two-line list rows and card rows.
    row_2: size."row_double",
    /// Minimum height of a settings card row.
    row_setting: size."row_setting",
    /// Pane header (title + toolbar).
    pane_header_h: size."pane_header",
    card_header_h: size."card_header",
    file_bar_h: size."file_bar",
    tab_bar_h: size."tab_bar",
    /// A segment inside a segmented control.
    segment_h: size."segment",
    chip_h: size."chip",
    badge_h: size."badge",
    /// Agent / identity avatar diameter.
    avatar_size: size."avatar",
    icon_md: size."icon",
    icon_sm: size."icon_sm",
    icon_xs: size."icon_xs",
}

/// Corner of a filled row flush against a card's border, so its fill
/// follows the card's curve instead of poking out at the corners.
pub(super) fn r_card_inner() -> f32 {
    (r_xl() - border_w()).max(0.)
}

/// Thin bars and grips: a third of the small radius.
pub(super) fn r_xs() -> f32 {
    (r_sm() / 3.).round()
}

pub(super) fn font_ui() -> SharedString {
    with_active(|active| active.font_ui.clone())
}

pub(super) fn mono() -> SharedString {
    with_active(|active| active.font_mono.clone())
}

/// Weight of labels and emphasised text.
pub(super) fn weight_medium() -> FontWeight {
    with_active(|active| active.weight_medium)
}

/// Weight of titles and headings.
pub(super) fn weight_semibold() -> FontWeight {
    with_active(|active| active.weight_semibold)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Palette {
    pub dark: bool,
    pub window: Rgba,
    pub window_border: Rgba,
    /// Main content area to the right of the sidebar.
    pub base: Rgba,
    pub sidebar: Rgba,
    /// Secondary column inside the main area: list panes and file trees.
    pub pane: Rgba,
    /// Cards.
    pub surface: Rgba,
    /// Opaque background for menus, popovers and dialogs.
    pub elevated: Rgba,
    /// Opaque fill for avatars and stacked tiles that overlap each other.
    pub avatar: Rgba,
    pub raised: Rgba,
    pub hover: Rgba,
    pub selected: Rgba,
    pub border: Rgba,
    pub border_strong: Rgba,
    pub text: Rgba,
    pub sidebar_text: Rgba,
    pub secondary: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    /// Border of a focused or open field.
    pub focus: Rgba,
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    /// Text on a danger button.
    pub on_danger: Rgba,
    pub danger_soft: Rgba,
    pub overlay: Rgba,
    /// `base`, `pane` and `surface` inside a dialog.
    pub dialog_base: Rgba,
    pub dialog_pane: Rgba,
    pub dialog_surface: Rgba,
    /// Switch colours; `None` keeps the component library's.
    pub switch_track: Option<Rgba>,
    pub switch_thumb: Option<Rgba>,
}

/// What a surface is, so a theme can give it the matching finish. The
/// order matches `crate::themes::MATERIALS`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Material {
    /// Cards and other resting panes.
    Pane,
    /// Menus, toasts and drag previews.
    Popover,
    /// Dialogs.
    Dialog,
    /// Secondary buttons, badges, chips.
    Control,
    /// The selected or hovered row, tab or segment.
    State,
    /// The primary button.
    Primary,
    /// Inputs, selects and pickers.
    Field,
    /// A groove that holds segments.
    Well,
    /// The window behind everything.
    Window,
    /// The sidebar column.
    Sidebar,
    /// The main area beside the sidebar.
    Content,
    /// List and file tree columns inside the main area.
    Column,
}

/// A material resolved for one colour mode. `None` keeps what the
/// component set.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Finish {
    pub fill: Option<Background>,
    pub border: Option<Rgba>,
    pub shadows: Option<Vec<BoxShadow>>,
    /// Only drawn by [`super::surface_art::SurfaceExt::surface`].
    pub texture: Option<TextureArt>,
    pub frame: Option<FrameArt>,
}

/// The active theme's finish of `material` in `p`'s colour mode.
pub(super) fn finish(p: Palette, material: Material) -> Finish {
    with_active(|active| {
        let mode = if p.dark { &active.dark } else { &active.light };
        mode.materials[material as usize].clone()
    })
}

impl Palette {
    /// Palette of the active theme.
    pub fn current(dark: bool) -> Self {
        with_active(|active| {
            if dark {
                active.dark.palette
            } else {
                active.light.palette
            }
        })
    }

    /// Palette for content drawn inside a dialog, whose ground differs from
    /// the page's in some themes.
    pub fn on_elevated(self) -> Self {
        Self {
            base: self.dialog_base,
            pane: self.dialog_pane,
            surface: self.dialog_surface,
            ..self
        }
    }

    /// Palette for code that only has the component theme mode at hand.
    pub fn for_mode(mode: ComponentThemeMode) -> Self {
        Self::current(mode.is_dark())
    }
}

/// Gives a surface the colours and shadows of the active theme's material.
/// Callers set the default fill, border and size first; the theme's material
/// then replaces what it defines. Works in hover and selected styles too;
/// surfaces that can carry image art use `SurfaceExt::surface` instead.
pub(super) trait MaterialExt: Styled + Sized {
    fn material(mut self, p: Palette, material: Material) -> Self {
        let finish = finish(p, material);
        if let Some(fill) = finish.fill {
            self = self.bg(fill);
        }
        if let Some(border) = finish.border {
            self = self.border_color(border);
        }
        if let Some(shadows) = finish.shadows {
            self = self.shadow(shadows);
        }
        self
    }
}

impl<T: Styled> MaterialExt for T {}

/// The asset path of the icon the active theme puts in place of the
/// built-in `name`.
pub(super) fn icon_override(name: &str) -> Option<SharedString> {
    with_active(|active| active.icons.get(name).cloned())
}

/// The file of the active theme's coloured replacement for icon `name`.
pub(super) fn color_icon(name: &str) -> Option<PathBuf> {
    with_active(|active| active.color_icons.get(name).cloned())
}

/// Font files already handed to the text system. Fonts can't be removed
/// again, so each file is added once.
static LOADED_FONTS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

fn load_fonts(files: &[PathBuf], cx: &App) {
    let Ok(mut loaded) = LOADED_FONTS.lock() else {
        return;
    };
    let mut fonts = Vec::new();
    for file in files {
        if !loaded.contains(file)
            && let Ok(bytes) = std::fs::read(file)
        {
            loaded.insert(file.clone());
            fonts.push(Cow::Owned(bytes));
        }
    }
    if !fonts.is_empty()
        && let Err(error) = cx.text_system().add_fonts(fonts)
    {
        eprintln!("kitter: failed to load theme fonts: {error}");
    }
}

/// Switch gpui-component's theme mode and align the library-drawn widgets
/// (context menus, checkboxes, tooltips, inputs) with Kitter's tokens.
pub(super) fn apply_component_theme(
    mode: ComponentThemeMode,
    theme_id: &str,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    let (active, fonts) = theme_file::resolve(theme_id);
    load_fonts(&fonts, cx);
    super::components::clear_color_icons();
    if let Ok(mut slot) = ACTIVE.write() {
        *slot = active;
    }
    ComponentTheme::change(mode, window, cx);
    let p = Palette::current(mode.is_dark());
    let theme = ComponentTheme::global_mut(cx);
    theme.radius = gpui::px(r_md());
    theme.radius_lg = gpui::px(r_modal());
    theme.font_family = font_ui();
    theme.mono_font_family = mono();
    // A ring-coloured border is enough; the outer halo reads as heavy.
    theme.focus_ring = false;
    let colors: [(&mut Hsla, Rgba); 14] = [
        (&mut theme.colors.background, p.base),
        (&mut theme.colors.foreground, p.text),
        (&mut theme.colors.border, p.border),
        (&mut theme.colors.input, p.border_strong),
        (&mut theme.colors.ring, p.focus),
        (&mut theme.colors.popover, p.elevated),
        (&mut theme.colors.popover_foreground, p.text),
        (&mut theme.colors.accent, p.hover),
        (&mut theme.colors.accent_foreground, p.text),
        (&mut theme.colors.muted_foreground, p.muted),
        (&mut theme.colors.primary, p.primary),
        (&mut theme.colors.primary_foreground, p.on_primary),
        (&mut theme.colors.danger, p.danger),
        (&mut theme.colors.list_active, p.selected),
    ];
    for (slot, color) in colors {
        *slot = color.into();
    }
    theme.tokens.popover = theme.colors.popover.into();
    theme.tokens.primary = theme.colors.primary.into();
    theme.tokens.accent = theme.colors.accent.into();
    if let Some(track) = p.switch_track {
        let track: Hsla = track.into();
        theme.tokens.switch = track.into();
    }
    if let Some(thumb) = p.switch_thumb {
        let thumb: Hsla = thumb.into();
        theme.tokens.switch_thumb = thumb.into();
    }
    // The base layer keeps its own copy for resize handles and scrollbars.
    ComponentTheme::sync_base(cx);
}
