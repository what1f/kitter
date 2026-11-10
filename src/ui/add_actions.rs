use super::*;

impl KitterApp {
    pub(super) fn browse_add_local(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("common.choose").to_string().into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    this.scan_local_folder(path, cx);
                });
            }
        })
        .detach();
    }

    pub(super) fn scan_local_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.add_flow.task.is_some() {
            return;
        }
        self.add_flow.task = Some(AddTask::Scanning);
        self.add_flow.scan = None;
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_chosen = false;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { source::scan_local(&path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_flow.task = None;
                match result {
                    Ok(scan) => {
                        this.add_flow.group_name = Some(scan.default_group_name());
                        this.add_flow.scan = Some(scan);
                    }
                    Err(error) => this.add_flow.error = Some(this.error_message(error)),
                }
                this.notify_dialog(cx);
            });
        })
        .detach();
    }

    pub(super) fn install_selected(&mut self, cx: &mut Context<Self>) {
        let Some(project) = (if self.install_flow.global {
            dirs::home_dir()
        } else {
            self.projects_view.open_project.clone()
        }) else {
            self.show_notice(t!("add.project_required").to_string(), cx);
            return;
        };
        let selected = self.selected_skill_keys();
        if selected.is_empty() {
            return;
        }
        let targets = crate::agent_plugins::AgentRegistry::global()
            .install_targets()
            .into_iter()
            .filter(|target| self.install_flow.selected_targets.contains(target))
            .collect::<Vec<_>>();
        let mut installed = 0usize;
        let mut failures = Vec::new();
        for storage_name in selected {
            let Some(skill) = self
                .model
                .skills
                .iter()
                .find(|skill| skill_storage_name(skill) == storage_name)
            else {
                continue;
            };
            match project::install_from_path(&project, &skill.path, &skill.record.name, &targets) {
                Ok(()) => installed += 1,
                Err(error) => failures.push(format!(
                    "{}: {}",
                    skill.record.name,
                    self.error_message(error)
                )),
            }
        }
        if installed > 0 && !self.install_flow.global {
            self.model.library.config.touch_project(&project);
            let _ = self.model.library.save();
        }
        if installed > 0 {
            // A global installation contributes to every project's effective
            // Skills, so invalidate all derived scans before redrawing.
            self.projects_view.context_estimates.borrow_mut().clear();
        }
        self.install_flow.modal = false;
        self.close_dialog(cx);
        let summary = if failures.is_empty() {
            if installed == 1 {
                t!("install.installed_single", count = installed).to_string()
            } else {
                crate::i18n::tn(
                    "common.installed_skills",
                    installed,
                    &[("count", installed.to_string())],
                )
            }
        } else {
            t!(
                "common.install_partial",
                count = installed,
                failed = failures.len()
            )
            .to_string()
        };
        self.refresh(cx);
        if failures.is_empty() {
            self.show_notice(summary, cx);
        } else {
            self.show_notice(format!("{}\n{}", summary, failures.join("\n")), cx);
        }
    }

    pub(super) fn scan_existing_skills(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.task.is_some() {
            return;
        }
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let projects = self.model.library.config.project_paths();
        let roots = if let Some(root) = &self.add_flow.adoption_root {
            vec![root.clone()]
        } else {
            let mut roots = vec![home.clone()];
            roots.extend(projects);
            roots
        };
        let library_dir = self.model.library.config.library_dir.clone();
        let managed = self.model.skills.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.add_flow.adoption_cancel = Some(cancel.clone());
        self.add_flow.adoption_scan = None;
        self.add_flow.selected.clear();
        self.add_flow.adoption_expanded.clear();
        self.add_flow.error = None;
        self.add_flow.task = Some(AddTask::Scanning);
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let worker_cancel = cancel.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    adoption::scan_roots(&home, &roots, &library_dir, &managed, &worker_cancel)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                // A cancelled/closed/reopened dialog must not receive stale results.
                if !this
                    .add_flow
                    .adoption_cancel
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &cancel))
                {
                    return;
                }
                this.add_flow.adoption_cancel = None;
                this.add_flow.task = None;
                match result {
                    Ok(scan) => {
                        this.add_flow.selected = scan.default_selection();
                        if scan.candidates.is_empty() {
                            this.add_flow.error = Some(t!("add.adoption_empty").to_string().into());
                        }
                        this.add_flow.adoption_scan = Some(Arc::new(scan));
                        this.reset_adoption_rows();
                    }
                    Err(error) => this.add_flow.error = Some(this.error_message(error)),
                }
                this.notify_dialog(cx);
            });
        })
        .detach();
    }

    pub(super) fn browse_adoption_root(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.task.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("common.choose").to_string().into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await {
                if let Some(path) = paths.first().cloned() {
                    let _ = this.update(cx, |this, cx| {
                        this.add_flow.adoption_root = Some(path);
                        this.add_flow.adoption_scan = None;
                        this.add_flow.selected.clear();
                        this.notify_dialog(cx);
                    });
                }
            }
        })
        .detach();
    }

    pub(super) fn adopt_selected_skills(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.task.is_some() || self.add_flow.selected.is_empty() {
            return;
        }
        let Some(scan) = self.add_flow.adoption_scan.clone() else {
            return;
        };
        let selected = self.add_flow.selected.clone();
        let data_dir = self.model.library.data_dir().to_path_buf();
        self.add_flow.task = Some(AddTask::Importing);
        self.add_flow.error = None;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut library = SkillLibrary::open_in(data_dir)?;
                    let mut succeeded = HashSet::new();
                    let mut failures = Vec::new();
                    let mut last = None;
                    for candidate in scan.candidates.iter().filter(|c| selected.contains(&c.id)) {
                        let identity = candidate.identity();
                        let result = scan
                            .variants(candidate)
                            .try_for_each(|c| c.verify())
                            .and_then(|()| {
                                library.adopt(candidate, &scan.references_for(candidate))
                            });
                        match result {
                            Ok(storage) => {
                                succeeded.insert(identity);
                                last = Some(storage);
                            }
                            Err(error) => failures.push(format!(
                                "{}: {}",
                                candidate.name,
                                messages::error_message(&error)
                            )),
                        }
                    }
                    Ok::<_, anyhow::Error>((library, succeeded, failures, last))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_flow.task = None;
                match result {
                    Ok((library, succeeded, failures, last)) => {
                        this.model.library = library;
                        if let Some(storage) = last {
                            this.skills_view.selection.select_one(storage);
                        }
                        if let Some(scan) = &mut this.add_flow.adoption_scan {
                            Arc::make_mut(scan).retain(|c| !succeeded.contains(&c.identity()));
                            this.add_flow.selected.retain(|id| scan.contains_id(id));
                        }
                        this.reset_adoption_rows();
                        this.refresh(cx);
                        if failures.is_empty() {
                            this.close_dialog(cx);
                            let message = crate::i18n::tn(
                                "common.adopted_skills",
                                succeeded.len(),
                                &[("count", (succeeded.len()).to_string())],
                            );
                            this.show_notice(message, cx);
                        } else {
                            this.add_flow.error = Some(failures.join("\n"));
                            this.notify_dialog(cx);
                        }
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

    pub(super) fn scan_add_source(&mut self, cx: &mut Context<Self>) {
        if self.add_flow.kind == AddKind::Existing {
            self.scan_existing_skills(cx);
            return;
        }
        let input_value = self.add_flow.primary_input.read(cx).value();
        if self.add_flow.task.is_some() || input_value.trim().is_empty() {
            return;
        }
        let kind = self.add_flow.kind;
        let input = input_value.trim().to_string();
        self.add_flow.task = Some(AddTask::Scanning);
        self.add_flow.scan = None;
        self.add_flow.selected.clear();
        self.add_flow.error = None;
        self.add_flow.group_enabled = true;
        self.add_flow.group_chosen = false;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match kind {
                        AddKind::Npx => source::scan_npx(&input),
                        AddKind::Claude => source::scan_claude(&input),
                        AddKind::Local => crate::user_bail!("errors.skill_folder_required"),
                        AddKind::Existing => unreachable!("handled before source scan"),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_flow.task = None;
                match result {
                    Ok(scan) => {
                        this.add_flow.group_name = Some(scan.default_group_name());
                        this.add_flow.scan = Some(scan);
                    }
                    Err(error) => this.add_flow.error = Some(this.error_message(error)),
                }
                this.notify_dialog(cx);
            });
        })
        .detach();
    }
}
