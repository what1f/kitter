use super::*;

impl KitterApp {
    pub(super) fn open_add_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_flow.task = None;
        self.add_flow.scan = None;
        self.add_flow.adoption_scan = None;
        self.add_flow.adoption_expanded.clear();
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_name = None;
        let english = self.uses_english();
        let placeholder = match self.add_flow.kind {
            AddKind::Claude => {
                if english {
                    "Plugin name or claude plugin install command"
                } else {
                    "插件名称或 claude plugin install 命令"
                }
            }
            AddKind::Npx => {
                if english {
                    "Paste a skills.sh/GitHub URL or npx skills add command"
                } else {
                    "粘贴 skills.sh、GitHub 地址或 npx skills add 命令"
                }
            }
            AddKind::Local | AddKind::Existing => "",
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
        if let Some(cancel) = self.add_flow.adoption_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.add_flow.adoption_scan = None;
        self.add_flow.adoption_expanded.clear();
        self.add_flow.kind = kind;
        let english = self.uses_english();
        let placeholder = match kind {
            AddKind::Claude => {
                if english {
                    "Plugin name or claude plugin install command"
                } else {
                    "插件名称或 claude plugin install 命令"
                }
            }
            AddKind::Npx => {
                if english {
                    "Paste a skills.sh/GitHub URL or npx skills add command"
                } else {
                    "粘贴 skills.sh、GitHub 地址或 npx skills add 命令"
                }
            }
            AddKind::Local | AddKind::Existing => "",
        };
        self.add_flow.primary_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(placeholder, window, cx);
        });
        self.add_flow.scan = None;
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_name = None;
        self.notify_dialog(cx);
    }

    pub(super) fn open_install_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.install_flow.modal = true;
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

    pub(super) fn check_all_updates(&mut self, cx: &mut Context<Self>) {
        if self.model.checking_updates {
            return;
        }
        self.model.checking_updates = true;
        let data_dir = self.model.library.data_dir().to_path_buf();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    let count = source::check_updates(&mut library)?;
                    Ok::<_, anyhow::Error>((library, count))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.model.checking_updates = false;
                let message = match result {
                    Ok((library, 0)) => {
                        this.model.library = library;
                        this.tr("所有技能都是最新版本", "All Skills are up to date")
                            .to_string()
                    }
                    Ok((library, count)) => {
                        this.model.library = library;
                        if this.uses_english() {
                            format!("{count} Skill(s) can be updated")
                        } else {
                            format!("发现 {count} 个可更新的技能")
                        }
                    }
                    Err(error) => error.to_string(),
                };
                this.show_notice(message, cx);
                this.refresh(cx);
            });
        })
        .detach();
    }

    pub(super) fn update_skill(&mut self, storage_name: String, cx: &mut Context<Self>) {
        if self.model.updating_skill.is_some() {
            return;
        }
        self.model.updating_skill = Some(storage_name.clone());
        let data_dir = self.model.library.data_dir().to_path_buf();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    source::update_by_storage(&mut library, &storage_name)?;
                    Ok::<_, anyhow::Error>(library)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.model.updating_skill = None;
                let message = match result {
                    Ok(library) => {
                        this.model.library = library;
                        this.tr("技能已更新", "Skill updated").to_string()
                    }
                    Err(error) => error.to_string(),
                };
                this.show_notice(message, cx);
                this.refresh(cx);
            });
        })
        .detach();
    }

    pub(super) fn import_scanned_skills(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.kind == AddKind::Existing {
            self.adopt_selected_skills(cx);
            return;
        }
        if self.add_flow.task.is_some() || self.add_flow.selected.is_empty() {
            return;
        }
        let Some(scan) = self.add_flow.scan.take() else {
            return;
        };
        let selected = self.add_flow.selected.clone();
        let group_name = (self.add_flow.selected.len() > 1 && self.add_flow.group_enabled)
            .then(|| self.add_flow.group_name.clone())
            .flatten();
        let data_dir = self.model.library.data_dir().to_path_buf();
        self.add_flow.task = Some(AddTask::Importing);
        self.add_flow.error = None;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    let count =
                        scan.import_selected(&mut library, &selected, group_name.as_deref())?;
                    Ok::<_, anyhow::Error>((library, count))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_flow.task = None;
                match result {
                    Ok((library, count)) => {
                        this.model.library = library;
                        this.add_flow.selected.clear();
                        this.close_dialog(cx);
                        let message = if this.uses_english() {
                            format!("Added {count} Skill(s)")
                        } else {
                            format!("已添加 {count} 个技能")
                        };
                        this.show_notice(message, cx);
                        this.refresh(cx);
                    }
                    Err(error) => {
                        this.add_flow.error = Some(error.to_string());
                        this.notify_dialog(cx);
                    }
                }
            });
        })
        .detach();
    }
}
