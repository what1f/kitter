use super::*;

impl KitterApp {
    pub(super) fn open_dialog(&mut self, kind: DialogKind, cx: &mut Context<Self>) {
        let app = cx.entity();
        self.shell.dialog_body = Some(cx.new(|_| DialogBody { app, kind }));
        cx.notify();
    }

    pub(super) fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if self
            .shell
            .dialog_body
            .as_ref()
            .is_some_and(|body| matches!(body.read(cx).kind, DialogKind::Add))
            && self.add_flow.task.is_some()
            && self.add_flow.adoption_cancel.is_none()
        {
            return;
        }
        if let Some(cancel) = self.add_flow.adoption_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
            self.add_flow.task = None;
        }
        self.install_flow.modal = false;
        self.delete_flow.confirmation = None;
        self.delete_flow.selected.clear();
        self.tags_flow.edit = None;
        self.tags_flow.drop_target = None;
        self.tags_flow.delete_pending = None;
        self.tags_flow.error = None;
        self.tags_flow.assignment_keys.clear();
        self.tags_flow.assignment_label = None;
        self.tags_flow.return_to_assignment = None;
        self.groups_flow.drop_target = None;
        self.groups_flow.edit = None;
        self.groups_flow.delete_pending = None;
        self.groups_flow.move_skills.clear();
        self.shell.dialog_body = None;
        cx.notify();
    }

    pub(super) fn notify_dialog(&self, cx: &mut Context<Self>) {
        if let Some(body) = &self.shell.dialog_body {
            body.update(cx, |_, cx| cx.notify());
        }
    }

    pub(super) fn show_notice(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.set_notice(message, false, cx);
    }

    pub(super) fn show_sticky_notice(
        &mut self,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.set_notice(message, true, cx);
    }

    fn set_notice(&mut self, message: impl Into<String>, sticky: bool, cx: &mut Context<Self>) {
        self.shell.notice_generation = self.shell.notice_generation.wrapping_add(1);
        let generation = self.shell.notice_generation;
        self.shell.notice = Some(message.into());
        cx.notify();
        if sticky {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = this.update(cx, |this, cx| {
                if this.shell.notice_generation == generation {
                    this.shell.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn set_update_check_progress(
        &mut self,
        progress: source::UpdateCheckProgress,
        cx: &mut Context<Self>,
    ) {
        if self.model.update_check.is_none() {
            return;
        }
        self.model.update_check = Some(progress);
        self.shell.notice = Some(self.update_check_notice());
        cx.notify();
    }

    pub(super) fn update_check_notice(&self) -> String {
        match &self.model.update_check {
            Some(progress) if progress.total > 0 => {
                if self.uses_english() {
                    format!("Checking updates {}/{}", progress.scanned, progress.total)
                } else {
                    format!("正在检查更新 {}/{}", progress.scanned, progress.total)
                }
            }
            _ => self
                .tr("正在检查更新…", "Checking for updates…")
                .to_string(),
        }
    }
}
