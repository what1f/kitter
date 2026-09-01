// Shared low-frequency animation clock, adapted from Waku's GPUI motion helper.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, EntityId, Global, IntoElement, RenderOnce, Svg, Transformation, Window,
    percentage,
};

// GPUI currently refreshes the window for each animation frame. A 30 fps loader
// leaves enough frame budget for typing, menus and scrolling while background
// scans are running.
const TICK: Duration = Duration::from_millis(33);
const LEASE: Duration = Duration::from_millis(300);
const SPIN_PERIOD: Duration = Duration::from_millis(900);

struct AnimationLease {
    until: Instant,
}

struct AnimationClock {
    epoch: Instant,
    leases: HashMap<EntityId, AnimationLease>,
    running: bool,
}

impl Global for AnimationClock {}

impl Default for AnimationClock {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            leases: HashMap::new(),
            running: false,
        }
    }
}

fn lease(view: EntityId, cx: &mut App) {
    let clock = cx.default_global::<AnimationClock>();
    clock.leases.insert(
        view,
        AnimationLease {
            until: Instant::now() + LEASE,
        },
    );
    if clock.running {
        return;
    }
    clock.running = true;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            let parked = cx.update(|cx| {
                let clock = cx.default_global::<AnimationClock>();
                let now = Instant::now();
                clock.leases.retain(|_, lease| lease.until > now);
                if clock.leases.is_empty() {
                    clock.running = false;
                    return true;
                }
                for view in clock.leases.keys().copied().collect::<Vec<_>>() {
                    cx.notify(view);
                }
                false
            });
            if parked {
                break;
            }
        }
    })
    .detach();
}

#[derive(IntoElement)]
struct Spinner {
    icon: Svg,
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let phase = if cx.reduce_motion() {
            0.0
        } else {
            let clock = cx.default_global::<AnimationClock>();
            let phase = (clock.epoch.elapsed().as_secs_f32() / SPIN_PERIOD.as_secs_f32()).fract();
            lease(window.current_view(), cx);
            phase
        };
        self.icon
            .with_transformation(Transformation::rotate(percentage(phase)))
    }
}

pub fn spin(icon: Svg) -> AnyElement {
    Spinner { icon }.into_any_element()
}
