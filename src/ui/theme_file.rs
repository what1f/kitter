//! Turns a [`crate::themes::Theme`] into what the desktop paints with. See
//! `docs/themes.md`.

use std::{collections::BTreeMap, path::PathBuf};

use gpui::{
    Background, BoxShadow, FontWeight, SharedString, linear_color_stop, linear_gradient, point, px,
    rgba,
};

use super::{
    surface_art::{FrameArt, TextureArt},
    theme::{Finish, Metrics, Palette},
};
use crate::themes::{self, MaterialStyle, Mode, Paint, Theme};

/// A theme the user can pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ThemeChoice {
    pub id: String,
    pub name: String,
}

pub(super) use crate::themes::themes_dir;

/// One colour mode of the active theme.
#[derive(Clone)]
pub(super) struct ActiveMode {
    pub palette: Palette,
    /// Indexed by [`super::theme::Material`].
    pub materials: [Finish; themes::MATERIALS.len()],
}

/// Everything the desktop reads from the applied theme.
#[derive(Clone)]
pub(super) struct ActiveTheme {
    pub light: ActiveMode,
    pub dark: ActiveMode,
    pub metrics: Metrics,
    pub font_ui: SharedString,
    pub font_mono: SharedString,
    pub weight_medium: FontWeight,
    pub weight_semibold: FontWeight,
    /// Asset paths of replaced icons, by icon name.
    pub icons: BTreeMap<String, SharedString>,
    /// Files of icons drawn in their own colours, by icon name.
    pub color_icons: BTreeMap<String, PathBuf>,
}

/// Built-in themes first, then readable user themes sorted by name.
pub(super) fn list_themes() -> Vec<ThemeChoice> {
    themes::list_in(&themes_dir())
        .into_iter()
        .map(|theme| ThemeChoice {
            id: theme.id,
            name: theme.name,
        })
        .collect()
}

pub(super) fn standard() -> ActiveTheme {
    from_theme(&themes::builtin(themes::STANDARD).expect("standard theme"))
}

/// The theme `id` ready to apply, and the font files it brings. Unknown ids
/// and unreadable themes fall back to the standard theme.
pub(super) fn resolve(id: &str) -> (ActiveTheme, Vec<PathBuf>) {
    let theme = themes::resolve_in(&themes_dir(), id);
    crate::assets::set_theme_icons(theme.icons.clone());
    (from_theme(&theme), theme.fonts.clone())
}

fn from_theme(theme: &Theme) -> ActiveTheme {
    let tokens = &theme.tokens;
    let weight = |weight: u16| FontWeight(f32::from(weight));
    ActiveTheme {
        light: ActiveMode::from_mode(&tokens.light, &theme.images),
        dark: ActiveMode::from_mode(&tokens.dark, &theme.images),
        metrics: Metrics::from_tokens(tokens),
        font_ui: tokens.font.ui.clone().into(),
        font_mono: tokens.font.mono.clone().into(),
        weight_medium: weight(tokens.font.medium),
        weight_semibold: weight(tokens.font.semibold),
        icons: theme
            .icons
            .keys()
            .map(|name| (name.clone(), crate::assets::theme_icon_path(name).into()))
            .collect(),
        color_icons: theme.color_icons.clone(),
    }
}

impl ActiveMode {
    fn from_mode(mode: &Mode, images: &BTreeMap<String, PathBuf>) -> Self {
        Self {
            palette: Palette::from_mode(mode),
            materials: themes::MATERIALS.map(|name| {
                mode.materials
                    .get(name)
                    .map(|style| Finish::from_style(style, images))
                    .unwrap_or_default()
            }),
        }
    }
}

fn background(paint: Paint) -> Background {
    match paint {
        Paint::Solid(color) => rgba(color).into(),
        Paint::Gradient { angle, from, to } => linear_gradient(
            angle,
            linear_color_stop(rgba(from.0), from.1),
            linear_color_stop(rgba(to.0), to.1),
        ),
    }
}

impl Finish {
    fn from_style(style: &MaterialStyle, images: &BTreeMap<String, PathBuf>) -> Self {
        Self {
            texture: style
                .texture
                .as_ref()
                .and_then(|texture| TextureArt::load(texture, images.get(&texture.image)?)),
            frame: style
                .frame
                .as_ref()
                .and_then(|frame| FrameArt::load(frame, images.get(&frame.image)?)),
            fill: style.fill.map(background),
            border: style.border.map(rgba),
            shadows: style.shadows.as_ref().map(|shadows| {
                shadows
                    .iter()
                    .map(|shadow| BoxShadow {
                        color: rgba(shadow.color).into(),
                        offset: point(px(shadow.x), px(shadow.y)),
                        blur_radius: px(shadow.blur),
                        spread_radius: px(shadow.spread),
                        inset: shadow.inset,
                    })
                    .collect()
            }),
        }
    }
}

impl Palette {
    pub(super) fn from_mode(mode: &Mode) -> Self {
        let get = |key: &str| mode.colors.get(key).copied().map(rgba);
        let color = |key: &str| get(key).unwrap_or_else(|| rgba(0));
        Self {
            dark: mode.dark,
            window: color("window"),
            window_border: color("window_border"),
            base: color("base"),
            sidebar: color("sidebar"),
            pane: color("pane"),
            surface: color("surface"),
            elevated: color("elevated"),
            avatar: color("avatar"),
            raised: color("raised"),
            hover: color("hover"),
            selected: color("selected"),
            border: color("border"),
            border_strong: color("border_strong"),
            text: color("text"),
            sidebar_text: color("sidebar_text"),
            secondary: color("secondary"),
            muted: color("muted"),
            accent: color("accent"),
            focus: color("focus"),
            primary: color("primary"),
            on_primary: color("on_primary"),
            success: color("success"),
            warning: color("warning"),
            danger: color("danger"),
            on_danger: color("on_danger"),
            danger_soft: color("danger_soft"),
            overlay: color("overlay"),
            dialog_base: color("dialog_base"),
            dialog_pane: color("dialog_pane"),
            dialog_surface: color("dialog_surface"),
            switch_track: get("switch_track"),
            switch_thumb: get("switch_thumb"),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, hsla};

    use super::*;
    use crate::ui::theme::Material;

    fn glass_light() -> ActiveMode {
        from_theme(&themes::builtin(themes::SOLID_GLASS).unwrap()).light
    }

    fn inset(y: f32, spread: f32, color: u32) -> BoxShadow {
        BoxShadow {
            inset: true,
            ..shadow(0., y, 0., spread, color)
        }
    }

    fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: u32) -> BoxShadow {
        BoxShadow {
            color: rgba(color).into(),
            offset: point(px(x), px(y)),
            blur_radius: px(blur),
            spread_radius: px(spread),
            inset: false,
        }
    }

    #[test]
    fn built_in_palettes_keep_their_colours() {
        let standard = standard();
        assert_eq!(standard.light.palette.window, rgba(0xf5f5f5ff));
        assert_eq!(standard.dark.palette.on_danger, rgba(0xffffffff));
        assert_eq!(standard.light.palette.on_elevated(), standard.light.palette);
        assert!(standard.light.palette.switch_track.is_none());
        assert_eq!(standard.metrics.r_md, 8.);
        assert_eq!(standard.metrics.border_w, 1.);
        assert_eq!(standard.font_ui.as_ref(), ".SystemUIFont");
        assert_eq!(standard.weight_semibold, FontWeight::SEMIBOLD);

        let glass = glass_light();
        let dialog = glass.palette.on_elevated();
        assert_eq!(dialog.base, glass.palette.elevated);
        assert_eq!(dialog.surface, rgba(0xf3f4f7ff));
        assert_eq!(glass.palette.switch_thumb, Some(rgba(0xffffffff)));
    }

    #[test]
    fn standard_materials_keep_the_flat_look() {
        let standard = standard().light;
        let popover = &standard.materials[Material::Popover as usize];
        let soft: Hsla = hsla(0., 0., 0., 0.1);
        let popover_shadows = popover.shadows.as_ref().unwrap();
        assert_eq!(popover_shadows.len(), 2);
        assert_eq!(popover_shadows[0].offset, point(px(0.), px(10.)));
        assert_eq!(popover_shadows[0].blur_radius, px(15.));
        assert_eq!(popover_shadows[0].spread_radius, px(-3.));
        assert!((popover_shadows[0].color.a - soft.a).abs() < 0.01);
        let dialog = &standard.materials[Material::Dialog as usize];
        assert_eq!(dialog.shadows.as_ref().unwrap()[0].blur_radius, px(50.));
        for material in [Material::Pane, Material::Control, Material::Field] {
            assert_eq!(standard.materials[material as usize], Finish::default());
        }
    }

    /// The finish Solid Glass had before materials became theme data. Inset
    /// shadows on bordered materials moved from `y + 1` to `spread + 1`,
    /// which paints the same line inside the border.
    #[test]
    fn solid_glass_materials_match_the_original_finish() {
        let glass = glass_light();
        let finish = |material: Material| glass.materials[material as usize].clone();
        assert_eq!(
            finish(Material::Pane),
            Finish {
                border: Some(rgba(0x3b42531a)),
                shadows: Some(vec![
                    inset(1., 1., 0xffffff73),
                    inset(0., 2., 0xffffff26),
                    shadow(0., 1., 2., 0., 0x1b21300f),
                    shadow(0., 12., 32., -14., 0x1f2d4c47),
                ]),
                ..Finish::default()
            }
        );
        assert_eq!(
            finish(Material::Field),
            Finish {
                fill: Some(rgba(0xffffff66).into()),
                border: Some(rgba(0x292e3824)),
                shadows: Some(vec![inset(1., 1., 0xffffff99)]),
                ..Finish::default()
            }
        );
        assert_eq!(
            finish(Material::State),
            Finish {
                fill: Some(rgba(0x292e3817).into()),
                shadows: Some(vec![inset(1., 0., 0xffffff66), inset(0., 1., 0x292e3821)]),
                ..Finish::default()
            }
        );
        assert_eq!(
            finish(Material::Well).shadows.unwrap()[1],
            shadow(0., 1., 0., 0., 0xffffffd9)
        );
        assert_eq!(finish(Material::Dialog), finish(Material::Popover));
    }
}
