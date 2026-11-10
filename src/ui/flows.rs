use super::*;

impl KitterApp {
    pub(super) fn open_add_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.add_flow.task.is_some() {
            return;
        }
        self.groups_flow.edit = None;
        self.add_flow.task = None;
        self.add_flow.scan = None;
        self.add_flow.adoption_scan = None;
        self.add_flow.adoption_expanded.clear();
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_chosen = false;
        self.add_flow.group_name = None;
        let placeholder = match self.add_flow.kind {
            AddKind::Claude => t!("add.plugin_placeholder").to_string(),
            AddKind::Npx => t!("add.source_placeholder").to_string(),
            AddKind::Local | AddKind::Existing => String::new(),
        };
        self.add_flow.primary_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(placeholder, window, cx);
        });
        self.open_dialog(DialogKind::Add, cx);
    }

    pub(super) fn set_add_kind(
        &mut self,
        kind: AddKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.add_flow.task.is_some() {
            return;
        }
        if let Some(cancel) = self.add_flow.adoption_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.add_flow.adoption_scan = None;
        self.add_flow.adoption_expanded.clear();
        self.add_flow.kind = kind;
        let placeholder = match kind {
            AddKind::Claude => t!("add.plugin_placeholder").to_string(),
            AddKind::Npx => t!("add.source_placeholder").to_string(),
            AddKind::Local | AddKind::Existing => String::new(),
        };
        self.add_flow.primary_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(placeholder, window, cx);
        });
        self.groups_flow.edit = None;
        self.tags_flow.error = None;
        self.add_flow.scan = None;
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_chosen = false;
        self.add_flow.group_name = None;
        self.notify_dialog(cx);
    }

    pub(super) fn open_install_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.install_flow.modal = true;
        self.install_flow.pending = None;
        self.install_flow.error = None;
        self.install_flow.global = self.projects_view.global_project_view;
        self.install_flow.selected_targets.clear();
        self.sync_project_select(window, cx);
        self.open_dialog(DialogKind::Install, cx);
    }

    pub(super) fn open_delete_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        self.delete_flow.busy = false;
        self.open_dialog(DialogKind::Delete, cx);
    }

    pub(super) fn start_background_update_check(&mut self, cx: &mut Context<Self>) {
        if self.model.update_check.is_some() || self.model.updating_batch.is_some() {
            return;
        }
        self.model.update_check = Some(source::UpdateCheckProgress::default());
        let data_dir = self.model.library.data_dir().to_path_buf();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    source::check_updates_if_due(&mut library)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.model.update_check = None;
                if let Err(error) = result {
                    // Startup checks are silent; keep diagnostics for debugging.
                    let _ = this.error_message(error);
                }
                if let Ok(library) = SkillLibrary::open_in(this.model.library.data_dir()) {
                    this.model.library = library;
                }
                this.refresh(cx);
                if let Some(request) = this.model.pending_updates.take() {
                    this.request_skill_updates(request, cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn request_skill_updates(
        &mut self,
        request: SkillUpdateRequest,
        cx: &mut Context<Self>,
    ) {
        if self.model.updating_batch.is_some()
            || self.model.updating_skill.is_some()
            || self.model.pending_updates.is_some()
        {
            return;
        }
        if self.model.update_check.is_some() {
            self.model.pending_updates = Some(request);
            self.show_sticky_notice(t!("updates.checking_notice").to_string(), cx);
            return;
        }
        let names = match request {
            SkillUpdateRequest::All => self
                .model
                .skills
                .iter()
                .map(|skill| skill_storage_name(skill).to_string())
                .collect::<Vec<_>>(),
            SkillUpdateRequest::Selection(names) => names,
        };
        let names = names
            .into_iter()
            .filter(|name| {
                self.model.skills.iter().any(|skill| {
                    skill_storage_name(skill) == name
                        && source::can_update(&self.model.library, &skill.record)
                })
            })
            .collect::<Vec<_>>();
        if names.is_empty() {
            self.show_notice(t!("updates.none").to_string(), cx);
            return;
        }
        let progress = source::UpdateProgress {
            total: names.len(),
            ..Default::default()
        };
        self.model.updating_batch = Some(progress.clone());
        self.model.updating_skill = names.first().cloned();
        self.show_sticky_notice(self.skill_update_notice(), cx);
        let data_dir = self.model.library.data_dir().to_path_buf();
        let progress = Arc::new(Mutex::new(progress));
        let worker_progress = progress.clone();
        let done = Arc::new(AtomicBool::new(false));
        let worker_done = done.clone();
        cx.spawn(async move |this, cx| {
            let worker = cx.background_executor().spawn(async move {
                let result = source::update_skills_with_progress(&data_dir, &names, |snapshot| {
                    *worker_progress
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = snapshot;
                });
                worker_done.store(true, Ordering::Release);
                result
            });
            while !done.load(Ordering::Acquire) {
                let snapshot = progress
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone();
                let _ = this.update(cx, |this, cx| {
                    this.model.updating_skill = snapshot.current.clone();
                    this.model.updating_batch = Some(snapshot);
                    this.shell.notice = Some(this.skill_update_notice());
                    cx.notify();
                });
                if done.load(Ordering::Acquire) {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
            }
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                this.model.updating_batch = None;
                this.model.updating_skill = None;
                if let Ok(library) = SkillLibrary::open_in(this.model.library.data_dir()) {
                    this.model.library = library;
                }
                let message = match result {
                    Ok(report) if !report.failures.is_empty() => t!(
                        "updates.batch_partial",
                        updated = report.updated,
                        failed = report.failures.len()
                    )
                    .to_string(),
                    Ok(report) if report.updated == 0 => t!("updates.none").to_string(),
                    Ok(report) => crate::i18n::tn(
                        "updates.updated_count",
                        report.updated,
                        &[("count", report.updated.to_string())],
                    ),
                    Err(error) => this.error_message(error),
                };
                this.show_notice(message, cx);
                this.refresh(cx);
            });
        })
        .detach();
    }

    pub(super) fn update_skill(&mut self, storage_name: String, cx: &mut Context<Self>) {
        self.request_skill_updates(SkillUpdateRequest::Selection(vec![storage_name]), cx);
    }

    pub(super) fn import_scanned_skills(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.kind == AddKind::Existing {
            self.adopt_selected_skills(cx);
            return;
        }
        if self.add_flow.task.is_some() || self.add_flow.selected.is_empty() {
            return;
        }
        if self.groups_flow.edit == Some(GroupEdit::Create) {
            self.commit_group_edit(cx);
            if self.groups_flow.edit.is_some() {
                return;
            }
        }
        let Some(scan) = self.add_flow.scan.take() else {
            return;
        };
        let selected = self.add_flow.selected.clone();
        let group_name = self.add_group_name();
        let local_mode = self.add_flow.local_mode;
        let data_dir = self.model.library.data_dir().to_path_buf();
        self.add_flow.task = Some(AddTask::Importing);
        self.add_flow.error = None;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    let summary = scan.import_selected_with_mode(
                        &mut library,
                        &selected,
                        group_name.as_deref(),
                        local_mode,
                    )?;
                    Ok::<_, anyhow::Error>((library, summary))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_flow.task = None;
                match result {
                    Ok((library, summary)) => {
                        this.model.library = library;
                        this.add_flow.selected.clear();
                        this.close_dialog(cx);
                        let message = match (summary.added, summary.skipped) {
                            (0, skipped) => t!("add.skipped_summary", count = skipped).to_string(),
                            (added, 0) => crate::i18n::tn(
                                "add.added_skills",
                                added,
                                &[("count", (added).to_string())],
                            ),
                            (added, skipped) => crate::i18n::tn(
                                "add.added_with_skips",
                                added,
                                &[
                                    ("count", (added).to_string()),
                                    ("skipped", (skipped).to_string()),
                                ],
                            ),
                        };
                        this.show_notice(message, cx);
                        this.refresh(cx);
                    }
                    Err(error) => {
                        this.add_flow.error = Some(this.error_message(error));
                        this.notify_dialog(cx);
                    }
                }
            });
        })
        .detach();
    }
}
