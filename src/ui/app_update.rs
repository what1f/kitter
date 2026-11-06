use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use super::*;
use crate::app_update::{self, Progress, Release};

#[derive(Default)]
pub(super) enum AppUpdate {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(u8),
    Installing,
    /// Installed, but the standalone CLI failed, so the restart waits for the user.
    Ready(app_update::DesktopUpdate),
}

impl KitterApp {
    /// Startup check: stays silent unless there is something to install.
    pub(super) fn start_app_update_check(&mut self, cx: &mut Context<Self>) {
        app_update::cleanup_previous();
        self.check_app_update(false, cx);
    }

    pub(super) fn app_update_available(&self) -> bool {
        matches!(self.app_update, AppUpdate::Available(_))
    }

    fn check_app_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        if manual {
            self.app_update = AppUpdate::Checking;
            cx.notify();
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { app_update::latest_release() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.app_update = match result {
                    Ok(release) if app_update::has_update(&release) => {
                        AppUpdate::Available(release)
                    }
                    Ok(_) if manual => AppUpdate::UpToDate,
                    Ok(_) => AppUpdate::Idle,
                    Err(error) => {
                        let message = this.error_message(error);
                        if manual {
                            this.show_notice(message, cx);
                        }
                        AppUpdate::Idle
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn install_app_update(&mut self, cx: &mut Context<Self>) {
        let AppUpdate::Available(release) = std::mem::take(&mut self.app_update) else {
            return;
        };
        self.app_update = AppUpdate::Downloading(0);
        cx.notify();
        let percent = Arc::new(AtomicU8::new(0));
        let installing = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let worker = cx.background_executor().spawn({
            let (percent, installing, done) = (percent.clone(), installing.clone(), done.clone());
            let release = release.clone();
            async move {
                let result = app_update::update_desktop(&release, |progress| match progress {
                    Progress::Downloading { done, total } => {
                        let value = (done * 100).checked_div(total).unwrap_or(0).min(100);
                        percent.store(value as u8, Ordering::Relaxed);
                    }
                    Progress::Installing => installing.store(true, Ordering::Relaxed),
                });
                done.store(true, Ordering::Release);
                result
            }
        });
        cx.spawn(async move |this, cx| {
            while !done.load(Ordering::Acquire) {
                let state = if installing.load(Ordering::Relaxed) {
                    AppUpdate::Installing
                } else {
                    AppUpdate::Downloading(percent.load(Ordering::Relaxed))
                };
                let _ = this.update(cx, |this, cx| {
                    this.app_update = state;
                    cx.notify();
                });
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
            }
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(update) => match &update.cli {
                        Some(Err(message)) => {
                            this.show_notice(
                                t!("settings.update_cli_failed", error = message).to_string(),
                                cx,
                            );
                            this.app_update = AppUpdate::Ready(update);
                        }
                        _ => this.restart_after_update(&update, cx),
                    },
                    Err(error) => {
                        let message = this.error_message(error);
                        this.show_notice(message, cx);
                        this.app_update = AppUpdate::Available(release);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn restart_after_update(&mut self, update: &app_update::DesktopUpdate, cx: &mut Context<Self>) {
        match app_update::relaunch(update) {
            Ok(()) => cx.quit(),
            Err(error) => {
                let message = self.error_message(error);
                self.show_notice(message, cx);
            }
        }
    }

    pub(super) fn about_group(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let control: AnyElement = match &self.app_update {
            AppUpdate::Idle => kit::button(
                p,
                "app-update-check",
                kit::ButtonKind::Secondary,
                kit::Size::Md,
                true,
            )
            .child(t!("settings.update_check").to_string())
            .on_click(cx.listener(|this, _, _, cx| this.check_app_update(true, cx)))
            .into_any_element(),
            AppUpdate::Checking => kit::button(
                p,
                "app-update-check",
                kit::ButtonKind::Secondary,
                kit::Size::Md,
                false,
            )
            .child(t!("settings.update_checking").to_string())
            .into_any_element(),
            AppUpdate::UpToDate => div()
                .text_size(px(fs_small()))
                .text_color(p.muted)
                .child(t!("settings.update_latest").to_string())
                .into_any_element(),
            AppUpdate::Available(release) => kit::button(
                p,
                "app-update-install",
                kit::ButtonKind::Primary,
                kit::Size::Md,
                true,
            )
            .child(t!("settings.update_install", version = release.version).to_string())
            .on_click(cx.listener(|this, _, _, cx| this.install_app_update(cx)))
            .into_any_element(),
            AppUpdate::Downloading(percent) => kit::button(
                p,
                "app-update-install",
                kit::ButtonKind::Primary,
                kit::Size::Md,
                false,
            )
            .child(t!("settings.update_downloading", percent = percent).to_string())
            .into_any_element(),
            AppUpdate::Installing => kit::button(
                p,
                "app-update-install",
                kit::ButtonKind::Primary,
                kit::Size::Md,
                false,
            )
            .child(t!("settings.update_installing").to_string())
            .into_any_element(),
            AppUpdate::Ready(_) => kit::button(
                p,
                "app-update-restart",
                kit::ButtonKind::Primary,
                kit::Size::Md,
                true,
            )
            .child(t!("settings.update_restart").to_string())
            .on_click(cx.listener(|this, _, _, cx| {
                if let AppUpdate::Ready(update) = std::mem::take(&mut this.app_update) {
                    this.restart_after_update(&update, cx);
                    this.app_update = AppUpdate::Ready(update);
                }
            }))
            .into_any_element(),
        };
        let changelog = kit::button(
            p,
            "app-changelog",
            kit::ButtonKind::Ghost,
            kit::Size::Md,
            true,
        )
        .child(t!("settings.changelog").to_string())
        .on_click(cx.listener(|this, _, _, cx| {
            let zh = crate::i18n::locale_for(this.model.library.config.language) == "zh-CN";
            cx.open_url(if zh {
                "https://kitter.dev/zh/changelog/"
            } else {
                "https://kitter.dev/changelog/"
            });
        }));
        kit::card(p).child(
            kit::setting_row(
                p,
                true,
                "Kitter",
                t!("settings.version", version = app_update::CURRENT_VERSION).to_string(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(changelog)
                    .child(control),
            ),
        )
    }
}
