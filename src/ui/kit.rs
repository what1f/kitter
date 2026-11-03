//! Kitter's base components. Every page builds on these instead of styling
//! raw `div()`s, so a control looks the same wherever it appears.
//!
//! Builders return plain GPUI elements; callers attach handlers and layout
//! (margins, width) but should not override the visual tokens set here.

use super::*;

pub(super) fn clear() -> Rgba {
    rgba(0x00000000)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Size {
    Sm,
    Md,
}

impl Size {
    fn height(self) -> f32 {
        match self {
            Size::Sm => h_sm(),
            Size::Md => h_md(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ButtonKind {
    /// The one confirming action of a dialog.
    Primary,
    /// Bordered; for actions that sit next to form controls.
    Secondary,
    /// Borderless; dismissive or low-emphasis actions.
    Ghost,
    /// Ghost with secondary text, for inline "+ add" style actions.
    Quiet,
    /// Destructive confirmation inside a confirm dialog only.
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum IconKind {
    Ghost,
    /// Bordered; pairs with an adjacent input or select.
    Secondary,
    /// Ghost that turns red on hover.
    Danger,
}

fn element_id(id: impl Into<ElementId>) -> (ElementId, String) {
    let id = id.into();
    let selector = id.to_string();
    (id, selector)
}

pub(super) fn button(
    p: Palette,
    id: impl Into<ElementId>,
    kind: ButtonKind,
    size: Size,
    enabled: bool,
) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    let base = div()
        .id(id)
        .debug_selector(|| selector)
        .h(px(size.height()))
        .px(px(if size == Size::Sm { 10. } else { 12. }))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(r_md()))
        .border(px(border_w()))
        .border_color(clear())
        .whitespace_nowrap()
        .text_size(px(fs_body()))
        .font_weight(if kind == ButtonKind::Quiet {
            FontWeight::NORMAL
        } else {
            weight_medium()
        });
    if !enabled {
        let fill = match kind {
            ButtonKind::Ghost | ButtonKind::Quiet => clear(),
            _ => p.raised,
        };
        return base.bg(fill).text_color(p.muted).cursor_default();
    }
    let base = base.cursor_pointer();
    match kind {
        ButtonKind::Primary => base
            .bg(p.primary)
            .text_color(p.on_primary)
            .surface(p, Material::Primary)
            .hover(|button| button.opacity(0.86)),
        ButtonKind::Secondary => base
            .bg(p.surface)
            .border_color(p.border_strong)
            .text_color(p.text)
            .surface(p, Material::Control)
            .hover(move |button| button.bg(p.hover)),
        ButtonKind::Ghost => base
            .text_color(p.text)
            .hover(move |button| button.bg(p.hover).material(p, Material::State)),
        ButtonKind::Quiet => base.text_color(p.secondary).hover(move |button| {
            button
                .bg(p.hover)
                .text_color(p.text)
                .material(p, Material::State)
        }),
        ButtonKind::Danger => base
            .bg(p.danger)
            .text_color(p.on_danger)
            .hover(|button| button.opacity(0.88)),
    }
}

pub(super) fn icon_button(
    p: Palette,
    id: impl Into<ElementId>,
    path: &'static str,
    kind: IconKind,
    size: Size,
) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    let group: SharedString = format!("icon-button-{selector}").into();
    let (rest, hover) = match kind {
        IconKind::Ghost => (p.secondary, p.text),
        IconKind::Secondary => (p.text, p.text),
        IconKind::Danger => (p.danger, p.danger),
    };
    let button = div()
        .id(id)
        .debug_selector(|| selector)
        .group(group.clone())
        .size(px(size.height()))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(r_md()))
        .border(px(border_w()))
        .border_color(clear())
        .cursor_pointer();
    let button = match kind {
        IconKind::Ghost => {
            button.hover(move |button| button.bg(p.hover).material(p, Material::State))
        }
        IconKind::Secondary => button
            .bg(p.surface)
            .border_color(p.border_strong)
            .surface(p, Material::Control)
            .hover(move |button| button.bg(p.hover)),
        IconKind::Danger => button.hover(move |button| button.bg(p.danger_soft)),
    };
    button.child(
        KitterApp::icon(path, icon_md(), rest)
            .group_hover(group, move |icon| icon.text_color(hover)),
    )
}

/// Close (×) button for dialog headers.
pub(super) fn close_button(p: Palette, id: impl Into<ElementId>, enabled: bool) -> Stateful<Div> {
    let button = icon_button(p, id, "icons/x.svg", IconKind::Ghost, Size::Sm);
    if enabled {
        button
    } else {
        button.opacity(0.4).cursor_default()
    }
}

/// Ghost icon button usable as a `Popover` trigger.
pub(super) fn icon_trigger(
    p: Palette,
    id: impl Into<ElementId>,
    path: &'static str,
    label: impl Into<SharedString>,
) -> gpui_base::Button {
    gpui_base::Button::new(id)
        .accessibility_label(label.into())
        .size(px(h_sm()))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(r_md()))
        .text_color(p.secondary)
        .hover(move |button| {
            button
                .bg(p.hover)
                .text_color(p.text)
                .material(p, Material::State)
        })
        .active(move |button| button.bg(p.selected).material(p, Material::State))
        .styles(|styles| styles.selected(|style| style.bg(p.selected).material(p, Material::State)))
        .child(KitterApp::icon(path, icon_md(), p.secondary))
}

/// Quiet text button usable as a `Popover` trigger; callers add an icon and
/// a label.
pub(super) fn quiet_trigger(p: Palette, id: impl Into<ElementId>) -> gpui_base::Button {
    let (id, selector) = element_id(id);
    gpui_base::Button::new(id)
        .debug_selector(|| selector)
        .h(px(h_sm()))
        .px(px(10.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(r_md()))
        .text_size(px(fs_body()))
        .text_color(p.secondary)
        .hover(move |button| {
            button
                .bg(p.hover)
                .text_color(p.text)
                .material(p, Material::State)
        })
}

/// Select-style trigger for a `Popover` menu.
pub(super) fn select_trigger(
    p: Palette,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
) -> gpui_base::Button {
    let label = label.into();
    gpui_base::Button::new(id)
        .accessibility_label(label.clone())
        .h(px(h_md()))
        .flex_shrink_0()
        .pl(px(10.))
        .pr(px(8.))
        .rounded(px(r_md()))
        .border(px(border_w()))
        .border_color(p.border_strong)
        .bg(p.surface)
        .material(p, Material::Field)
        .text_color(p.text)
        .font_family(font_ui())
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .hover(move |button| button.bg(p.hover))
        .styles(|styles| {
            styles
                .selected(|style| style.border_color(p.focus))
                .disabled(|style| style.bg(p.surface).text_color(p.muted))
        })
        // Button::label clips text to a 1em line box in gpui-component.
        // Own the label so descenders fit while long values still ellipsize.
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(fs_body()))
                .line_height(relative(1.5))
                .child(label),
        )
        .child(KitterApp::icon("icons/chevron-down.svg", icon_sm(), p.muted).flex_none())
}

/// Field-shaped button that opens a file or folder chooser.
pub(super) fn picker(
    p: Palette,
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    div()
        .id(id)
        .debug_selector(|| selector)
        .h(px(h_md()))
        .w_full()
        .min_w_0()
        .px(px(10.))
        .rounded(px(r_md()))
        .border(px(border_w()))
        .border_color(p.border_strong)
        .bg(p.surface)
        .surface(p, Material::Field)
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(fs_body()))
        .text_color(p.secondary)
        .child(KitterApp::icon(icon, icon_md(), p.muted))
        .child(div().min_w_0().flex_1().truncate().child(label.into()))
        .when(enabled, |picker| {
            picker
                .cursor_pointer()
                .hover(move |picker| picker.bg(p.hover))
        })
        .when(!enabled, |picker| picker.opacity(0.55))
}

pub(super) fn input(p: Palette, state: &Entity<InputState>) -> Input {
    Input::new(state)
        .h(px(h_md()))
        .w_full()
        .rounded(px(r_md()))
        .bg(p.surface)
        .border_color(p.border_strong)
        .material(p, Material::Field)
        .text_size(px(fs_body()))
}

/// Multi-line text field that grows with its content.
pub(super) fn textarea(
    p: Palette,
    state: &Entity<gpui_component::input::TextareaState>,
) -> gpui_component::input::Textarea {
    gpui_component::input::Textarea::new(state)
        .w_full()
        .rounded(px(r_md()))
        .bg(p.surface)
        .border_color(p.border_strong)
        .material(p, Material::Field)
        .text_size(px(fs_body()))
}

pub(super) fn search_input(p: Palette, state: &Entity<InputState>) -> Input {
    input(p, state).prefix(KitterApp::icon("icons/search.svg", icon_md(), p.muted))
}

pub(super) fn field_label(p: Palette, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(fs_small()))
        .font_weight(weight_medium())
        .text_color(p.secondary)
        .child(text.into())
}

/// Labelled form field. Stack fields in `form()`.
pub(super) fn field(p: Palette, label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field_label(p, label))
        .child(control)
}

pub(super) fn form() -> Div {
    div().flex().flex_col().gap(px(16.))
}

pub(super) fn field_error(p: Palette, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(fs_small()))
        .text_color(p.danger)
        .child(text.into())
}

pub(super) fn section_title(text: impl Into<SharedString>) -> Div {
    div()
        .mb(px(8.))
        .ml(px(2.))
        .text_size(px(fs_body()))
        .font_weight(weight_semibold())
        .child(text.into())
}

/// Popover / dropdown panel. Callers set the width.
pub(super) fn menu(p: Palette) -> Div {
    div()
        .min_w(px(180.))
        .p(px(4.))
        .flex()
        .flex_col()
        .rounded(px(r_lg()))
        .border(px(border_w()))
        .border_color(p.border_strong)
        .bg(p.elevated)
        .surface(p, Material::Popover)
        .font_family(font_ui())
        .text_color(p.text)
}

fn menu_row(p: Palette, id: impl Into<ElementId>) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    div()
        .id(id)
        .debug_selector(|| selector)
        .w_full()
        .min_w_0()
        .px(px(8.))
        .rounded(px(r_sm()))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .text_size(px(fs_body()))
        .hover(move |row| row.bg(p.hover).material(p, Material::State))
}

fn menu_check(p: Palette) -> super::components::ThemeIcon {
    KitterApp::icon("icons/check.svg", icon_sm(), p.text).flex_none()
}

/// Single-line menu row; a selected row shows a trailing check mark.
pub(super) fn menu_item(
    p: Palette,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    menu_row(p, id)
        .h(px(h_sm()))
        .child(div().min_w_0().flex_1().truncate().child(label.into()))
        .when(selected, |row| row.child(menu_check(p)))
}

/// Menu row with a leading icon, for action menus.
pub(super) fn menu_action(
    p: Palette,
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    menu_row(p, id)
        .h(px(h_sm()))
        .child(KitterApp::icon(icon, icon_md(), p.secondary))
        .child(div().min_w_0().flex_1().truncate().child(label.into()))
}

/// Two-line menu row: a title and a muted monospace detail such as a path.
pub(super) fn menu_item_detail(
    p: Palette,
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    menu_row(p, id)
        .py(px(6.))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(div().truncate().child(title.into()))
                .child(
                    div()
                        .mt(px(2.))
                        .font_family(mono())
                        .text_size(px(fs_caption()))
                        .text_color(p.muted)
                        .truncate()
                        .child(detail.into()),
                ),
        )
        .when(selected, |row| row.child(menu_check(p)))
}

pub(super) fn menu_separator(p: Palette) -> Div {
    div().h(px(1.)).mx(px(-4.)).my(px(4.)).bg(p.border)
}

pub(super) fn menu_label(p: Palette, text: impl Into<SharedString>) -> Div {
    div()
        .px(px(8.))
        .pt(px(6.))
        .pb(px(4.))
        .text_size(px(fs_caption()))
        .font_weight(weight_medium())
        .text_color(p.muted)
        .child(text.into())
}

/// Container for mutually exclusive choices such as the theme mode.
pub(super) fn segmented(p: Palette) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(r_md()))
        .bg(p.raised)
        .surface(p, Material::Well)
}

pub(super) fn segment(
    p: Palette,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    div()
        .id(id)
        .debug_selector(|| selector)
        .h(px(segment_h()))
        .px(px(10.))
        .rounded(px(r_sm()))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(px(fs_body()))
        .when(active, |segment| {
            segment
                .bg(p.elevated)
                .border(px(border_w()))
                .border_color(p.border)
                .surface(p, Material::State)
                .text_color(p.text)
                .font_weight(weight_medium())
        })
        .when(!active, |segment| {
            segment
                .text_color(p.secondary)
                .hover(move |segment| segment.text_color(p.text))
        })
        .child(label.into())
}

/// Tab that switches a content area. Callers add the label (and count).
pub(super) fn tab(p: Palette, id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    div()
        .id(id)
        .debug_selector(|| selector)
        .h(px(h_sm()))
        .px(px(10.))
        .flex_none()
        .rounded(px(r_md()))
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .text_size(px(fs_body()))
        .when(active, |tab| {
            tab.bg(p.selected)
                .surface(p, Material::State)
                .text_color(p.text)
                .font_weight(weight_medium())
        })
        .when(!active, |tab| {
            tab.text_color(p.secondary)
                .hover(move |tab| tab.bg(p.hover).text_color(p.text))
        })
}

pub(super) fn tabs() -> Div {
    div().flex().flex_none().items_center().gap(px(4.))
}

/// Small status pill. Callers add an optional 12px icon and a label.
pub(super) fn badge(p: Palette) -> Div {
    div()
        .h(px(badge_h()))
        .px(px(6.))
        .rounded(px(r_sm()))
        .bg(p.raised)
        .surface(p, Material::Control)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .text_size(px(fs_caption()))
        .text_color(p.muted)
}

/// Tag-like token, optionally removable.
pub(super) fn chip(p: Palette) -> Div {
    div()
        .h(px(chip_h()))
        .px(px(8.))
        .rounded(px(r_sm()))
        .bg(p.raised)
        .surface(p, Material::Control)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .text_size(px(fs_small()))
        .text_color(p.secondary)
}

pub(super) fn count(p: Palette, value: impl ToString) -> Div {
    div()
        .flex_none()
        .font_family(mono())
        .text_size(px(fs_small()))
        .text_color(p.muted)
        .child(value.to_string())
}

/// Circular agent avatar. Identity marks are always round.
pub(super) fn avatar(p: Palette, path: &'static str) -> Div {
    div()
        .size(px(avatar_size()))
        .flex_none()
        .rounded(px(r_pill()))
        .bg(p.avatar)
        .flex()
        .items_center()
        .justify_center()
        .child(KitterApp::icon(path, icon_md(), p.text))
}

pub(super) fn agent_avatar(p: Palette, agent: &crate::agent_plugins::AgentDef) -> Div {
    div()
        .size(px(avatar_size()))
        .flex_none()
        .rounded(px(r_pill()))
        .bg(p.avatar)
        .text_color(p.text)
        .flex()
        .items_center()
        .justify_center()
        .child(KitterApp::brand_icon(agent, icon_md(), p.dark))
}

/// Rounded-square tile behind a functional icon (not an identity mark).
pub(super) fn icon_tile(p: Palette, path: &'static str, size: f32) -> Div {
    let (radius, icon) = if size >= 48. {
        (r_xl(), 22.)
    } else if size >= 40. {
        (r_lg(), 20.)
    } else {
        (r_sm(), icon_md())
    };
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(radius))
        .bg(p.raised)
        .flex()
        .items_center()
        .justify_center()
        .child(KitterApp::icon(path, icon, p.secondary))
}

pub(super) fn card(p: Palette) -> Div {
    div()
        .w_full()
        .rounded(px(r_xl()))
        .border(px(border_w()))
        .border_color(p.border)
        .bg(p.surface)
        .surface(p, Material::Pane)
        .overflow_hidden()
}

/// Row inside a card; every row after the first draws a divider.
pub(super) fn card_row(p: Palette, first: bool) -> Div {
    div()
        .min_h(px(row_2()))
        .px(px(16.))
        .flex()
        .items_center()
        .gap(px(12.))
        .text_size(px(fs_body()))
        .when(first, |row| row.rounded_t(px(r_card_inner())))
        .when(!first, |row| {
            row.border_t(px(divider_w())).border_color(p.border)
        })
}

/// Trailing "add" row that closes a card list.
pub(super) fn card_add_row(p: Palette, id: impl Into<ElementId>) -> Stateful<Div> {
    card_row(p, false)
        .id(id)
        .min_h(px(44.))
        .rounded_b(px(r_card_inner()))
        .cursor_pointer()
        .text_color(p.secondary)
        .hover(move |row| row.bg(p.hover))
}

/// Small heading row at the top of a card section.
pub(super) fn card_header(p: Palette, text: impl Into<SharedString>) -> Div {
    div()
        .h(px(card_header_h()))
        .px(px(16.))
        .flex()
        .items_center()
        .rounded_t(px(r_card_inner()))
        .bg(p.raised)
        .text_size(px(fs_small()))
        .font_weight(weight_medium())
        .text_color(p.secondary)
        .child(text.into())
}

/// Selectable list row (navigation, skill list, project list).
pub(super) fn list_row(p: Palette, id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    let (id, selector) = element_id(id);
    div()
        .id(id)
        .debug_selector(|| selector)
        .w_full()
        .min_w_0()
        .min_h(px(row_1()))
        .px(px(8.))
        .rounded(px(r_md()))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .when(selected, |row| {
            row.bg(p.selected).surface(p, Material::State)
        })
        .when(!selected, |row| row.hover(move |row| row.bg(p.hover)))
}

// Surfaces. Pages lay out with these instead of painting palette colors on
// raw divs, so a style only has to change them here.

/// Selected and hover fills for rows, tiles and triggers that are not built
/// from `list_row`.
pub(super) trait FillExt: Styled + Sized {
    /// Selected fill; leaves the element as-is when `selected` is false.
    fn selected_fill(self, p: Palette, selected: bool) -> Self {
        if selected {
            self.bg(p.selected).material(p, Material::State)
        } else {
            self
        }
    }

    /// Highlight for a drop target while something is dragged over it.
    fn drop_fill(self, p: Palette) -> Self {
        self.selected_fill(p, true)
    }

    /// Ring around avatars that overlap, cut from the ground behind them.
    fn avatar_ring(self, p: Palette) -> Self {
        self.border(px(border_w())).border_color(p.pane)
    }
}

impl<T: Styled> FillExt for T {}

pub(super) trait HoverFillExt: InteractiveElement + Styled + Sized {
    fn hover_fill(self, p: Palette) -> Self {
        self.hover(move |style| style.bg(p.hover))
    }
}

impl<T: InteractiveElement + Styled> HoverFillExt for T {}

/// Root of the window, behind the sidebar and the main area.
pub(super) fn window_ground(p: Palette) -> Div {
    div()
        .size_full()
        .relative()
        .font_family(font_ui())
        .text_color(p.text)
        .text_size(px(fs_emph()))
        .bg(p.window)
        .surface(p, Material::Window)
        .flex()
}

/// Main content area to the right of the sidebar.
pub(super) fn main_area(p: Palette) -> Div {
    div()
        .min_w_0()
        .flex_1()
        .min_h_0()
        .h_full()
        .bg(p.base)
        .surface(p, Material::Content)
        .flex()
        .flex_col()
}

/// App sidebar column.
pub(super) fn sidebar_panel(p: Palette) -> Div {
    div()
        .w_full()
        .min_w_0()
        .min_h_0()
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .bg(p.sidebar)
        .surface(p, Material::Sidebar)
}

/// Secondary column inside the main area: list panes and file trees.
/// Callers set the width.
pub(super) fn pane(p: Palette) -> Div {
    div()
        .min_w_0()
        .min_h_0()
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .bg(p.pane)
        .surface(p, Material::Column)
}

/// Title bar at the top of a pane.
pub(super) fn pane_header(p: Palette) -> Div {
    div()
        .h(px(pane_header_h()))
        .flex_none()
        .px(px(12.))
        .flex()
        .items_center()
        .border_b(px(divider_w()))
        .border_color(p.border)
}

/// Path bar above a file preview or editor.
pub(super) fn file_bar(p: Palette) -> Div {
    div()
        .h(px(file_bar_h()))
        .flex_none()
        .px(px(16.))
        .border_b(px(divider_w()))
        .border_color(p.border)
        .bg(p.pane)
        .flex()
        .items_center()
        .font_family(mono())
        .text_size(px(fs_small()))
        .text_color(p.secondary)
}

/// Tab strip under a detail header.
pub(super) fn tab_bar(p: Palette) -> Div {
    tabs()
        .h(px(tab_bar_h()))
        .px(px(24.))
        .border_b(px(divider_w()))
        .border_color(p.border)
}

/// Quiet tinted box for a path or a short list inside a dialog or pane.
pub(super) fn inset_box(p: Palette) -> Div {
    div().rounded(px(r_md())).bg(p.raised)
}

/// Bordered tile that holds one choice, such as a path to delete.
pub(super) fn option_tile(p: Palette) -> Div {
    div()
        .rounded(px(r_md()))
        .border(px(border_w()))
        .border_color(p.border)
        .bg(p.surface)
        .surface(p, Material::Control)
}

/// Tinted box that explains what a destructive action will do.
pub(super) fn danger_note(p: Palette) -> Div {
    div()
        .p(px(12.))
        .rounded(px(r_md()))
        .bg(p.danger_soft)
        .text_size(px(fs_body()))
}

/// Groove that a progress fill runs in.
pub(super) fn track(p: Palette) -> Div {
    div()
        .relative()
        .w_full()
        .h(px(4.))
        .rounded(px(r_xs()))
        .bg(p.raised)
        .surface(p, Material::Well)
        .overflow_hidden()
}

/// Fade over the last line of clamped text, matching the ground it sits on.
/// It switches to the hover fill while `group` is hovered.
pub(super) fn clamp_fade(p: Palette, group: &'static str) -> Div {
    let fade = |color: Rgba| {
        linear_gradient(
            0.,
            linear_color_stop(color, 0.),
            linear_color_stop(color.opacity(0.), 1.),
        )
    };
    div()
        .absolute()
        .left(px(0.))
        .right(px(0.))
        .bottom(px(0.))
        .h(px(18.))
        .bg(fade(p.base))
        .group_hover(group, move |style| style.bg(fade(p.hover)))
}

/// Six-dot grip shown on a draggable row.
pub(super) fn grip(p: Palette) -> Div {
    let column = || {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children((0..3).map(|_| div().size(px(3.)).rounded(px(r_pill())).bg(p.muted)))
    };
    div().flex().gap(px(2.)).child(column()).child(column())
}

/// Line that marks where a dragged row will land. Callers pin it to the
/// top or bottom edge.
pub(super) fn drop_line(p: Palette) -> Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(2.))
        .rounded(px(r_pill()))
        .bg(p.accent)
}

/// Dimmed backdrop behind a dialog.
pub(super) fn scrim(p: Palette) -> Div {
    div()
        .absolute()
        .top(px(0.))
        .right(px(0.))
        .bottom(px(0.))
        .left(px(0.))
        .bg(p.overlay)
        .flex()
        .items_center()
        .justify_center()
}

/// Frame of a dialog; paints the background and clips the corners.
pub(super) fn dialog_frame(p: Palette) -> Div {
    div()
        .max_h(px(680.))
        .rounded(px(r_modal()))
        .border(px(border_w()))
        .border_color(p.border_strong)
        .bg(p.elevated)
        .surface(p, Material::Dialog)
        .overflow_hidden()
}

pub(super) fn modal_header(title: impl Into<SharedString>, close: impl IntoElement) -> Div {
    div()
        .min_h(px(pane_header_h()))
        .pl(px(20.))
        .pr(px(16.))
        .pt(px(16.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_family(font_ui())
                .text_size(px(fs_title()))
                .font_weight(weight_semibold())
                .child(title.into()),
        )
        .child(close)
}

pub(super) fn modal_body() -> Div {
    div().px(px(20.)).pt(px(16.)).pb(px(20.))
}

pub(super) fn modal_footer(p: Palette) -> Div {
    div()
        .flex_none()
        .px(px(16.))
        .py(px(12.))
        .border_t(px(divider_w()))
        .border_color(p.border)
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.))
}

/// Dialog content. The dialog frame paints the background and clips the
/// corners, so the panel itself stays transparent.
pub(super) fn modal_panel(_p: Palette) -> Div {
    div().relative().flex().flex_col().font_family(font_ui())
}

pub(super) fn toast(p: Palette) -> Div {
    div()
        .px(px(12.))
        .py(px(10.))
        .rounded(px(r_lg()))
        .border(px(border_w()))
        .border_color(p.border_strong)
        .bg(p.elevated)
        .surface(p, Material::Popover)
        .flex()
        .flex_col()
        .gap(px(6.))
        .text_size(px(fs_body()))
}

pub(super) fn progress_bar(p: Palette, ratio: f32) -> Div {
    track(p).child(
        div()
            .h_full()
            .rounded(px(r_xs()))
            .bg(p.text)
            .w(relative(ratio.clamp(0., 1.))),
    )
}

/// Vertical scrollbar that fades in while scrolling and can be dragged.
/// Add it as a sibling of the scrolling element inside a `relative()` box;
/// inside the scrolling element it would scroll away with the content.
pub(super) fn scrollbar<H>(id: impl Into<ElementId>, handle: &H) -> Div
where
    H: gpui_component::scroll::ScrollbarHandle + Clone + 'static,
{
    div().absolute().inset_0().child(
        gpui_component::scroll::Scrollbar::vertical(handle)
            .id(id)
            .viewport_from_layout(),
    )
}

/// Centered placeholder for empty lists.
pub(super) fn empty_state(p: Palette, icon: &'static str, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.))
        .text_size(px(fs_body()))
        .text_color(p.muted)
        .child(KitterApp::icon(icon, 24., p.muted))
        .child(text.into())
}

/// Centered placeholder filling an empty detail pane; callers append a
/// hint or a call to action.
pub(super) fn empty_page(p: Palette, icon: &'static str, title: impl Into<SharedString>) -> Div {
    div()
        .relative()
        .min_w_0()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(icon_tile(p, icon, 48.))
        .child(
            div()
                .mt(px(16.))
                .text_size(px(fs_title()))
                .font_weight(weight_semibold())
                .child(title.into()),
        )
}

/// Title at the top of a list pane.
pub(super) fn pane_title(title: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(px(fs_emph()))
        .font_weight(weight_semibold())
        .child(title.into())
}

/// Floating label that follows the cursor while dragging.
/// `stacked` draws a second card behind the label when several items move.
pub(super) fn drag_preview(cx: &App, text: impl Into<SharedString>, stacked: bool) -> Div {
    let p = Palette::for_mode(cx.theme().mode);
    let card = || {
        div()
            .rounded(px(r_md()))
            .border(px(border_w()))
            .border_color(p.border_strong)
            .bg(p.elevated)
            .surface(p, Material::Popover)
    };
    let label = card()
        .px(px(10.))
        .py(px(6.))
        .text_color(p.text)
        .font_family(mono())
        .text_size(px(fs_small()))
        .child(text.into());
    if !stacked {
        return label;
    }
    div()
        .relative()
        .pr(px(4.))
        .pb(px(4.))
        .child(
            card()
                .absolute()
                .top(px(4.))
                .left(px(4.))
                .right_0()
                .bottom_0(),
        )
        .child(label)
}

/// Settings card row: title and description on the left, control on the right.
pub(super) fn setting_row(
    p: Palette,
    first: bool,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> Div {
    div()
        .min_h(px(row_setting()))
        .px(px(16.))
        .py(px(12.))
        .flex()
        .items_center()
        .gap(px(24.))
        .when(first, |row| row.rounded_t(px(r_card_inner())))
        .when(!first, |row| {
            row.border_t(px(divider_w())).border_color(p.border)
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .text_size(px(fs_body()))
                        .font_weight(weight_medium())
                        .child(title.into()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(fs_small()))
                        .text_color(p.muted)
                        .child(description.into()),
                ),
        )
}

/// Single-choice list row inside a dialog; the chosen row shows a check.
pub(super) fn choice_row(
    p: Palette,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    list_row(p, id, false)
        .text_size(px(fs_body()))
        .child(div().min_w_0().flex_1().truncate().child(label.into()))
        .when(selected, |row| row.child(menu_check(p)))
}

/// Top of a detail pane: icon tile, title, subtitle and trailing actions.
pub(super) fn detail_header(
    p: Palette,
    icon: &'static str,
    title: impl IntoElement,
    subtitle: impl IntoElement,
    actions: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(icon_tile(p, icon, 40.))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(fs_display()))
                        .font_weight(weight_semibold())
                        .line_height(relative(1.2))
                        .child(title),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(fs_small()))
                        .text_color(p.muted)
                        .truncate()
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .child(actions),
        )
}

/// Avatar for fetched candidates which do not yet have a registry definition.
pub(super) fn agent_icon_avatar(
    p: Palette,
    icon: &crate::agent_plugins::AgentIcon,
    name: &str,
) -> Div {
    use crate::agent_plugins::AgentIcon;
    let mark = match icon {
        AgentIcon::Monogram => div()
            .child(name.chars().next().unwrap_or_default().to_string())
            .into_any_element(),
        AgentIcon::Builtin(path) => {
            let path = *path;
            let dark = p.dark;
            img(ImageSource::Custom(Arc::new(move |_, cx| {
                let dark_path = format!("{}.dark.svg", path.trim_end_matches(".svg"));
                let bytes = dark
                    .then(|| cx.asset_source().load(&dark_path).ok().flatten())
                    .flatten()
                    .or_else(|| cx.asset_source().load(path).ok().flatten())?;
                Some(
                    cx.svg_renderer()
                        .render_single_frame(bytes.as_ref(), 2.0)
                        .map_err(ImageCacheError::from),
                )
            })))
            .size(px(KitterApp::brand_icon_size(path, icon_md())))
            .flex_none()
            .into_any_element()
        }
        AgentIcon::Inline { light, dark } => {
            let bytes = if p.dark {
                dark.as_ref().unwrap_or(light)
            } else {
                light
            }
            .clone();
            img(ImageSource::Custom(Arc::new(move |_, cx| {
                Some(
                    cx.svg_renderer()
                        .render_single_frame(bytes.as_ref(), 2.0)
                        .map_err(ImageCacheError::from),
                )
            })))
            .size(px(icon_md()))
            .flex_none()
            .into_any_element()
        }
    };
    div()
        .size(px(avatar_size()))
        .flex_none()
        .rounded(px(r_pill()))
        .bg(p.avatar)
        .text_color(p.text)
        .flex()
        .items_center()
        .justify_center()
        .child(mark)
}
