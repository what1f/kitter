use gpui::{Rgba, rgb, rgba};

pub(super) const MONO: &str = "JetBrains Mono";
pub(super) const FONT_UI: &str = ".SystemUIFont";
pub(super) const DESCRIPTION_MAX_WIDTH: f32 = 700.;
pub(super) const DESCRIPTION_COLLAPSED_LINES: usize = 3;
pub(super) const SETTINGS_CONTENT_MAX_WIDTH: f32 = 768.;
pub(super) const ROW_HEIGHT: f32 = 30.;
pub(super) const CONTROL_HEIGHT: f32 = 28.;
pub(super) const DIALOG_CONTROL_HEIGHT: f32 = 32.;
pub(super) const INPUT_HEIGHT: f32 = 40.;
pub(super) const SEARCH_HEIGHT: f32 = 32.;
pub(super) const RADIUS_CONTROL: f32 = 12.5;
pub(super) const RADIUS_LIST_ROW: f32 = 10.;
pub(super) const RADIUS_INLINE_INPUT: f32 = 10.;
pub(super) const RADIUS_MENU: f32 = 15.;
pub(super) const RADIUS_CARD: f32 = 20.;
pub(super) const RADIUS_MODAL: f32 = 25.;
pub(super) const CONTEXT_TOKEN_WARNING_THRESHOLD: usize = 2_000;
pub(super) const CONTEXT_TOKEN_DANGER_THRESHOLD: usize = 5_000;
pub(super) const CONTEXT_COUNT_WARNING_THRESHOLD: usize = 20;
pub(super) const CONTEXT_COUNT_DANGER_THRESHOLD: usize = 50;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub window: Rgba,
    pub window_border: Rgba,
    pub base: Rgba,
    pub sidebar: Rgba,
    pub surface: Rgba,
    pub elevated: Rgba,
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
    pub accent_fill: Rgba,
    pub on_accent: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub danger_soft: Rgba,
    pub overlay: Rgba,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            window: rgb(0x141414),
            window_border: rgba(0xb0b0b04d),
            base: rgb(0x181818),
            sidebar: rgb(0x181818),
            surface: rgb(0x181818),
            elevated: rgba(0x363636f5),
            raised: rgba(0xffffff0d),
            hover: rgba(0xffffff0f),
            selected: rgba(0xffffff0f),
            border: rgba(0xffffff15),
            border_strong: rgba(0xffffff28),
            text: rgb(0xdfdfdf),
            sidebar_text: rgba(0xffffffd9),
            secondary: rgba(0xffffffb5),
            muted: rgba(0xffffff7f),
            accent: rgb(0x339cff),
            accent_fill: rgb(0x0d0d0d),
            on_accent: rgb(0xffffff),
            success: rgb(0x3fb950),
            warning: rgb(0xe3b341),
            danger: rgb(0xff6762),
            danger_soft: rgba(0xff67621a),
            overlay: rgba(0x00000022),
        }
    }

    pub fn light() -> Self {
        Self {
            window: rgb(0xf5f5f5),
            window_border: rgba(0x64646459),
            base: rgb(0xffffff),
            sidebar: rgba(0xffffffb3),
            surface: rgb(0xffffff),
            elevated: rgba(0xfffffff5),
            raised: rgba(0x0d0d0d0d),
            hover: rgba(0x0d0d0d0e),
            selected: rgba(0x0d0d0d0e),
            border: rgba(0x0d0d0d14),
            border_strong: rgba(0x0d0d0d1e),
            text: rgb(0x0d0d0d),
            sidebar_text: rgba(0x0d0d0dd9),
            secondary: rgba(0x0d0d0db1),
            muted: rgba(0x0d0d0d7e),
            accent: rgb(0x0169cc),
            accent_fill: rgb(0x0d0d0d),
            on_accent: rgb(0xffffff),
            success: rgb(0x1a7f37),
            warning: rgb(0xc98700),
            danger: rgb(0xe02e2a),
            danger_soft: rgba(0xe02e2a1a),
            overlay: rgba(0x00000022),
        }
    }
}
