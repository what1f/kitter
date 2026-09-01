use gpui::Window;

#[cfg(target_os = "macos")]
use std::cell::RefCell;

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSView, NSWindow};

pub fn start_window_move(window: &Window) {
    window.start_window_move();
}

pub fn titlebar_double_click(window: &Window) {
    #[cfg(target_os = "macos")]
    window.titlebar_double_click();

    #[cfg(not(target_os = "macos"))]
    if window.window_controls().maximize && window.is_resizable() {
        window.zoom_window();
    }
}

#[cfg(target_os = "macos")]
struct SoftShadowWindow {
    parent: usize,
    window: Retained<NSWindow>,
    border_view: Retained<NSView>,
    ring_views: Vec<Retained<NSView>>,
}

#[cfg(target_os = "macos")]
thread_local! {
    static SOFT_SHADOW_WINDOW: RefCell<Option<SoftShadowWindow>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
const SHADOW_MARGIN: f64 = 36.0;
#[cfg(target_os = "macos")]
const SHADOW_RINGS: usize = 28;
#[cfg(target_os = "macos")]
const SHADOW_OFFSET_Y: f64 = -4.0;
#[cfg(target_os = "macos")]
const SHADOW_PEAK_ALPHA: f64 = 0.10;
/// Match Waku's macOS window material setup, then replace AppKit's hard-edged
/// shadow with a mouse-transparent child window for the soft shadow and border.
#[cfg(target_os = "macos")]
pub fn configure_window_material(window: &Window, dark: bool) {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode, NSWindowStyleMask,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let Some(_main_thread) = MainThreadMarker::new() else {
        return;
    };

    unsafe {
        let view = handle.ns_view.cast::<NSView>().as_ref();
        let Some(native_window) = view.window() else {
            return;
        };

        let background = if dark {
            NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.25)
        } else {
            NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.0)
        };
        native_window.setBackgroundColor(Some(&background));
        native_window.setHasShadow(false);

        let Some(content_view) = native_window.contentView() else {
            return;
        };
        for subview in content_view.subviews().iter() {
            let Some(effect_view) = subview.downcast_ref::<NSVisualEffectView>() else {
                continue;
            };
            effect_view.setMaterial(NSVisualEffectMaterial::Sidebar);
            effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect_view.setState(NSVisualEffectState::Active);
        }

        let parent = native_window.as_ref() as *const NSWindow as usize;
        let border_color = if dark {
            NSColor::colorWithSRGBRed_green_blue_alpha(
                176.0 / 255.0,
                176.0 / 255.0,
                176.0 / 255.0,
                0.30,
            )
        } else {
            NSColor::colorWithSRGBRed_green_blue_alpha(
                100.0 / 255.0,
                100.0 / 255.0,
                100.0 / 255.0,
                0.24,
            )
        };
        let border_width = 1.0 / native_window.backingScaleFactor();
        SOFT_SHADOW_WINDOW.with_borrow_mut(|slot| {
            if let Some(shadow) = slot.as_ref().filter(|shadow| shadow.parent == parent) {
                if let Some(layer) = shadow.border_view.layer() {
                    layer.setBorderColor(Some(&border_color.CGColor()));
                    layer.setBorderWidth(border_width);
                    layer.setCornerRadius(14.0 + border_width);
                }
                return;
            }

            let mut shadow_frame = native_window.frame();
            shadow_frame.origin.x -= SHADOW_MARGIN;
            shadow_frame.origin.y -= SHADOW_MARGIN;
            shadow_frame.size.width += SHADOW_MARGIN * 2.0;
            shadow_frame.size.height += SHADOW_MARGIN * 2.0;

            let shadow_window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(_main_thread),
                shadow_frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            );
            shadow_window.setOpaque(false);
            shadow_window.setBackgroundColor(Some(&NSColor::clearColor()));
            shadow_window.setHasShadow(false);
            shadow_window.setIgnoresMouseEvents(true);

            let Some(shadow_content) = shadow_window.contentView() else {
                return;
            };
            let mut ring_views = Vec::with_capacity(SHADOW_RINGS + 1);
            for expansion in 0..=SHADOW_RINGS {
                let expansion = expansion as f64;
                let mut ring_frame = shadow_content.bounds();
                ring_frame.origin.x += SHADOW_MARGIN - expansion;
                ring_frame.origin.y += SHADOW_MARGIN - expansion + SHADOW_OFFSET_Y;
                ring_frame.size.width -= (SHADOW_MARGIN - expansion) * 2.0;
                ring_frame.size.height -= (SHADOW_MARGIN - expansion) * 2.0;

                let ring_view = NSView::initWithFrame(NSView::alloc(_main_thread), ring_frame);
                ring_view.setWantsLayer(true);
                if let Some(layer) = ring_view.layer() {
                    let profile = |distance: f64| {
                        SHADOW_PEAK_ALPHA * (-(distance * distance) / (2.0 * 10.0 * 10.0)).exp()
                    };
                    let alpha = if expansion == SHADOW_RINGS as f64 {
                        profile(expansion)
                    } else {
                        1.0 - (1.0 - profile(expansion)) / (1.0 - profile(expansion + 1.0))
                    };
                    let color = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, alpha);
                    layer.setBackgroundColor(Some(&color.CGColor()));
                    layer.setCornerRadius(14.0 + expansion);
                }
                shadow_content.addSubview(&ring_view);
                ring_views.push(ring_view);
            }

            let mut border_frame = shadow_content.bounds();
            border_frame.origin.x += SHADOW_MARGIN - border_width;
            border_frame.origin.y += SHADOW_MARGIN - border_width;
            border_frame.size.width -= (SHADOW_MARGIN - border_width) * 2.0;
            border_frame.size.height -= (SHADOW_MARGIN - border_width) * 2.0;
            let border_view = NSView::initWithFrame(NSView::alloc(_main_thread), border_frame);
            border_view.setWantsLayer(true);
            if let Some(layer) = border_view.layer() {
                layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
                layer.setBorderColor(Some(&border_color.CGColor()));
                layer.setBorderWidth(border_width);
                layer.setCornerRadius(14.0 + border_width);
            }
            shadow_content.addSubview(&border_view);

            shadow_window.orderFront(None);
            native_window.addChildWindow_ordered(&shadow_window, NSWindowOrderingMode::Below);
            shadow_window
                .orderWindow_relativeTo(NSWindowOrderingMode::Below, native_window.windowNumber());
            *slot = Some(SoftShadowWindow {
                parent,
                window: shadow_window,
                border_view,
                ring_views,
            });
        });
    }
}

#[cfg(target_os = "macos")]
pub fn update_soft_window_shadow(window: &Window) {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    unsafe {
        let view = handle.ns_view.cast::<NSView>().as_ref();
        let Some(native_window) = view.window() else {
            return;
        };
        let parent = native_window.as_ref() as *const NSWindow as usize;
        SOFT_SHADOW_WINDOW.with_borrow_mut(|slot| {
            let Some(shadow) = slot.as_mut().filter(|shadow| shadow.parent == parent) else {
                return;
            };
            let mut frame = native_window.frame();
            frame.origin.x -= SHADOW_MARGIN;
            frame.origin.y -= SHADOW_MARGIN;
            frame.size.width += SHADOW_MARGIN * 2.0;
            frame.size.height += SHADOW_MARGIN * 2.0;
            shadow.window.setFrame_display(frame, false);

            let Some(content_view) = shadow.window.contentView() else {
                return;
            };
            let border_width = 1.0 / native_window.backingScaleFactor();
            let mut border_frame = content_view.bounds();
            border_frame.origin.x += SHADOW_MARGIN - border_width;
            border_frame.origin.y += SHADOW_MARGIN - border_width;
            border_frame.size.width -= (SHADOW_MARGIN - border_width) * 2.0;
            border_frame.size.height -= (SHADOW_MARGIN - border_width) * 2.0;
            shadow.border_view.setFrame(border_frame);
            if let Some(layer) = shadow.border_view.layer() {
                layer.setBorderWidth(border_width);
                layer.setCornerRadius(14.0 + border_width);
            }
            for (expansion, ring_view) in shadow.ring_views.iter().enumerate() {
                let expansion = expansion as f64;
                let mut ring_frame = content_view.bounds();
                ring_frame.origin.x += SHADOW_MARGIN - expansion;
                ring_frame.origin.y += SHADOW_MARGIN - expansion + SHADOW_OFFSET_Y;
                ring_frame.size.width -= (SHADOW_MARGIN - expansion) * 2.0;
                ring_frame.size.height -= (SHADOW_MARGIN - expansion) * 2.0;
                ring_view.setFrame(ring_frame);
            }
        });
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_window_material(_: &Window, _: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn update_soft_window_shadow(_: &Window) {}
