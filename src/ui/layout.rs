//! Resizable desktop panes, adapted from desktop-ui's DesktopShell.
use super::*;
use gpui_component::{h_resizable, resizable::ResizableState, resizable_panel};

pub(super) const SIDEBAR_WIDTH: f32 = 180.;
pub(super) const LIST_WIDTH: f32 = 280.;

fn pane(content: impl IntoElement) -> Div {
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .child(content)
}

pub(super) fn shell(
    state: &Entity<ResizableState>,
    sidebar: impl IntoElement,
    content: impl IntoElement,
) -> impl IntoElement {
    h_resizable("kitter-shell")
        .with_state(state)
        .child(
            resizable_panel()
                .size(px(SIDEBAR_WIDTH))
                .size_range(px(160.)..px(260.))
                .flex_none()
                .child(pane(sidebar)),
        )
        .child(
            resizable_panel()
                .size_range(px(600.)..Pixels::MAX)
                .child(pane(content)),
        )
}

pub(super) fn content(
    state: &Entity<ResizableState>,
    available_width: f32,
    list: impl IntoElement,
    detail: impl IntoElement,
) -> Div {
    div().size_full().min_w_0().min_h_0().child(
        h_resizable("kitter-content-panes")
            .with_state(state)
            .child(
                resizable_panel()
                    .size(px(LIST_WIDTH))
                    // Reserve room for detail even after a wide list is
                    // followed by shrinking the native window.
                    .size_range(px(220.)..px((available_width - 380.).clamp(220., 420.)))
                    .flex_none()
                    .child(pane(list)),
            )
            .child(
                resizable_panel()
                    .size_range(px(380.)..Pixels::MAX)
                    .child(pane(detail)),
            ),
    )
}
