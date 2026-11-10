use super::*;

impl KitterApp {
    /// A preset without skills has nothing to install globally.
    pub(super) fn install_flow_allows_global(&self) -> bool {
        self.install_flow
            .preset
            .as_deref()
            .and_then(|id| self.presets_view.store.preset(id).ok())
            .is_none_or(|preset| !preset.skills.is_empty())
    }

    pub(super) fn install_modal(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.install_flow.pending.is_some() {
            return self.install_replace_modal(cx);
        }
        let p = self.palette();
        let global_allowed = self.install_flow_allows_global();
        let has_target = !self.install_flow.selected_targets.is_empty()
            && if self.install_flow.global {
                global_allowed && dirs::home_dir().is_some()
            } else {
                self.projects_view.open_project.is_some()
            };
        let selected_keys = self.selected_skill_keys();
        let selected_count = selected_keys.len();
        let preset_name = self
            .install_flow
            .preset
            .as_deref()
            .and_then(|id| self.presets_view.store.preset(id).ok())
            .map(|preset| preset.name.clone());
        let selected_label = if let Some(name) = preset_name {
            name
        } else if selected_count > 1 {
            crate::i18n::tn(
                "common.skill_count",
                selected_count,
                &[("count", (selected_count).to_string())],
            )
        } else {
            self.selected_skill()
                .map(|skill| skill.record.name.clone())
                .unwrap_or_else(|| t!("install.skill").to_string())
        };
        let global_label = t!("common.global").to_string();
        let project_label = if self.install_flow.global {
            global_label.clone()
        } else {
            self.projects_view
                .open_project
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| t!("install.project_placeholder").to_string())
        };
        let selected_global = self.install_flow.global;
        let selected_project = self.projects_view.open_project.clone();
        let home = dirs::home_dir();
        let project_choices = home
            .iter()
            .filter(|_| global_allowed)
            .cloned()
            .map(|path| (true, path))
            .chain(
                self.model
                    .library
                    .config
                    .project_paths()
                    .into_iter()
                    .filter(|path| Some(path) != home.as_ref())
                    .map(|path| (false, path)),
            )
            .collect::<Vec<_>>();
        let app = cx.entity().downgrade();
        let project_control = Popover::new("install-project-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(kit::select_trigger(p, "install-project-select", project_label).w_full())
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(520.));
                for (index, (global, path)) in project_choices.iter().cloned().enumerate() {
                    let active = if global {
                        selected_global
                    } else {
                        !selected_global && selected_project.as_ref() == Some(&path)
                    };
                    let name = if global {
                        global_label.clone()
                    } else {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    };
                    let path_label = display_path(&path);
                    let app = app.clone();
                    menu = menu.child(
                        kit::menu_item_detail(
                            p,
                            ElementId::Name(format!("install-project-option-{index}").into()),
                            name,
                            path_label,
                            active,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.install_flow.global = global;
                                    if !global {
                                        this.projects_view.open_project = Some(path.clone());
                                    }
                                    this.notify_dialog(cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            });
        let project_picker = div().flex_1().child(project_control);
        let close_button = kit::close_button(p, "close-install-modal", true).on_click(cx.listener(
            |this, _, _, cx| {
                this.install_flow.modal = false;
                this.close_dialog(cx);
            },
        ));
        let install_content = kit::modal_body()
            .id("install-modal-content")
            .max_h(px(500.))
            .overflow_y_scroll()
            .overflow_x_hidden()
            .child(
                kit::form()
                    .child(kit::field(
                        p,
                        t!("install.install_to").to_string(),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(project_picker)
                            .child(
                                self.bordered_icon_button("modal-browse", "icons/folder.svg", cx)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.browse_project(window, cx)
                                        }),
                                    ),
                            ),
                    ))
                    .child(kit::field(
                        p,
                        t!("install.install_location").to_string(),
                        self.target_group(cx),
                    )),
            );
        let cancel = kit::button(
            p,
            "cancel-install",
            kit::ButtonKind::Ghost,
            kit::Size::Md,
            true,
        )
        .child(t!("common.cancel").to_string())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.install_flow.modal = false;
                this.close_dialog(cx);
            }),
        );
        let confirm = kit::button(
            p,
            "confirm-install",
            kit::ButtonKind::Primary,
            kit::Size::Md,
            has_target,
        )
        .child(t!("common.install").to_string())
        .when(has_target, |button| {
            button.on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.prepare_install(cx);
                }),
            )
        });
        let install_content = install_content
            .when_some(self.install_flow.error.clone(), |body, error| {
                body.child(div().text_color(p.danger).child(error))
            });
        let panel = kit::modal_panel(p)
            .child(kit::modal_header(
                t!("install.title", name = selected_label.clone()).to_string(),
                close_button,
            ))
            .child(install_content)
            .child(kit::modal_footer(p).child(cancel).child(confirm));
        let _ = window;
        panel
    }

    fn prepare_install(&mut self, cx: &mut Context<Self>) {
        self.install_flow.error = None;
        let result = (|| -> anyhow::Result<project::InstallPlan> {
            let project = if self.install_flow.global {
                dirs::home_dir()
            } else {
                self.projects_view.open_project.clone()
            }
            .ok_or_else(|| anyhow::anyhow!(t!("add.project_required").to_string()))?;
            let targets = self
                .install_flow
                .selected_targets
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let keys = if let Some(id) = &self.install_flow.preset {
                self.presets_view.store.preset(id)?.skills.clone()
            } else {
                self.selected_skill_keys()
            };
            let mut plan = project::InstallPlan::default();
            for key in keys {
                let skill = self
                    .model
                    .skills
                    .iter()
                    .find(|skill| skill_storage_name(skill) == key)
                    .ok_or_else(|| anyhow::anyhow!(t!("errors.skill_missing").to_string()))?;
                plan.add(&project, &skill.path, &skill.record.name, &targets)?;
            }
            Ok(plan)
        })();
        match result {
            Ok(plan) if plan.conflicts().next().is_some() => self.install_flow.pending = Some(plan),
            Ok(plan) => self.execute_install_plan(plan, cx),
            Err(error) => self.install_flow.error = Some(self.error_message(error)),
        }
        self.notify_dialog(cx);
        cx.notify();
    }

    fn execute_install_plan(&mut self, plan: project::InstallPlan, cx: &mut Context<Self>) {
        if self.install_flow.preset.is_some() {
            self.install_preset(plan, cx);
            return;
        }
        match plan.execute() {
            Ok(()) => self.install_selected(cx),
            Err(error) => {
                self.install_flow.error = Some(self.error_message(error));
                self.notify_dialog(cx);
                cx.notify();
            }
        }
    }

    fn install_replace_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let plan = self.install_flow.pending.as_ref().unwrap();
        let names = plan
            .conflicts()
            .map(|conflict| conflict.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut entries = kit::card(p);
        for (index, name) in names.into_iter().enumerate() {
            entries = entries.child(
                kit::card_row(p, index == 0)
                    .child(Self::icon("icons/package.svg", icon_md(), p.secondary))
                    .child(kit::field_label(p, name).flex_1().min_w_0().truncate()),
            );
        }
        let rows = kit::form()
            .child(kit::field_label(
                p,
                t!("install.overwrite_warning").to_string(),
            ))
            .child(
                div()
                    .id("install-conflicts")
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .child(entries),
            );
        self.action_dialog(
            "install-replace",
            t!("install.replace_title").to_string(),
            rows,
            t!("install.replace_and_install").to_string(),
            true,
            true,
            t!("common.back").to_string(),
            |this, cx| {
                this.install_flow.pending = None;
                this.install_flow.error = None;
                this.notify_dialog(cx);
                cx.notify();
            },
            |this, cx| {
                if let Some(plan) = this.install_flow.pending.take() {
                    this.execute_install_plan(plan, cx);
                }
            },
            cx,
        )
    }

    pub(super) fn toggle_install_target(&mut self, target: InstallTarget) {
        if !self.install_flow.selected_targets.remove(&target) {
            self.install_flow.selected_targets.insert(target);
        }
    }

    pub(super) fn target_group(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let universal = self
            .install_flow
            .selected_targets
            .contains(&InstallTarget::universal());
        let agent_cell = |agent: &crate::agent_plugins::AgentDef, right_border: bool| {
            div()
                .h(px(48.))
                // Keep the three-column grid stable when the final row has
                // fewer than three Agents.
                .flex_basis(relative(1. / 3.))
                .flex_shrink_0()
                .px(px(12.))
                .border_t(px(divider_w()))
                .border_color(p.border)
                .when(right_border, |cell| {
                    cell.border_r(px(divider_w())).border_color(p.border)
                })
                .flex()
                .items_center()
                .gap(px(8.))
                .child(kit::agent_avatar(p, agent))
                .child(
                    div()
                        .text_size(px(fs_small()))
                        .text_color(p.secondary)
                        .child(agent.name.clone()),
                )
        };
        let destination = if self.install_flow.global {
            dirs::home_dir()
        } else {
            self.projects_view.open_project.clone()
        };
        let readers = destination
            .as_ref()
            .and_then(|path| self.project_snapshot(path, cx))
            .map(|snapshot| snapshot.universal_agents.clone())
            .unwrap_or_default();
        let universal_agents = crate::agent_plugins::AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| readers.contains(&agent.id))
            .collect::<Vec<_>>();
        let mut universal_rows = div().flex().flex_col();
        for chunk in universal_agents.chunks(3) {
            let mut row = div().flex();
            for (index, agent) in chunk.iter().enumerate() {
                row = row.child(agent_cell(agent, index % 3 < 2));
            }
            for index in chunk.len()..3 {
                row = row.child(
                    div()
                        .h(px(48.))
                        .flex_basis(relative(1. / 3.))
                        .flex_shrink_0()
                        .border_t(px(divider_w()))
                        .border_color(p.border)
                        .when(index < 2, |cell| {
                            cell.border_r(px(divider_w())).border_color(p.border)
                        }),
                );
            }
            universal_rows = universal_rows.child(row);
        }
        let universal_box = kit::card(p)
            .child(
                div()
                    .id("target-universal")
                    .debug_selector(|| "target-universal".into())
                    .h(px(row_2()))
                    .px(px(16.))
                    .rounded_t(px(r_card_inner()))
                    .cursor_pointer()
                    .selected_fill(p, universal)
                    .flex()
                    .items_center()
                    .child(Checkbox::new("target-universal-checkbox").checked(universal))
                    .child(
                        div()
                            .ml(px(10.))
                            .text_size(px(fs_body()))
                            .font_weight(weight_medium())
                            .child(t!("install.universal").to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .text_color(p.muted)
                            .child(if self.install_flow.global {
                                "~/.agents/skills"
                            } else {
                                ".agents/skills"
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_install_target(InstallTarget::universal());
                        this.notify_dialog(cx);
                    })),
            )
            .child(universal_rows);
        let mut independent_rows = div().flex().flex_col();
        let independent = crate::agent_plugins::AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| {
                destination.as_ref().is_some_and(|destination| {
                    crate::agents::installation_root(
                        destination,
                        &InstallTarget::from(agent.id.as_str()),
                    )
                    .is_some()
                })
            })
            .collect::<Vec<_>>();
        let last = independent.len().saturating_sub(1);
        for (index, agent) in independent.into_iter().enumerate() {
            independent_rows = independent_rows.child(
                self.target_row(agent, cx)
                    .when(index == last, |row| row.rounded_b(px(r_card_inner()))),
            );
        }
        let independent_box = kit::card(p)
            .mt(px(8.))
            .child(kit::card_header(
                p,
                t!("install.agent_specific").to_string(),
            ))
            .child(independent_rows);
        div().child(universal_box).child(independent_box)
    }

    pub(super) fn target_row(
        &self,
        agent: &'static crate::agent_plugins::AgentDef,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let target = InstallTarget::from(agent.id.as_str());
        let id: SharedString = format!("target-{target}").into();
        let active = self.install_flow.selected_targets.contains(&target);
        div()
            .id(id.clone())
            .debug_selector({
                let id = id.clone();
                move || id.to_string()
            })
            .h(px(row_2()))
            .px(px(16.))
            .border_t(px(divider_w()))
            .border_color(p.border)
            .selected_fill(p, active)
            .flex()
            .items_center()
            .cursor_pointer()
            .child(Checkbox::new(ElementId::Name(format!("{id}-checkbox").into())).checked(active))
            .child(kit::agent_avatar(p, agent).ml(px(10.)))
            .child(
                div()
                    .ml(px(10.))
                    .text_size(px(fs_body()))
                    .child(agent.name.clone()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .font_family(mono())
                    .text_size(px(fs_small()))
                    .text_color(p.muted)
                    .child(if self.install_flow.global {
                        dirs::home_dir()
                            .and_then(|home| target.global_dir(&home))
                            .map(|root| display_path(&root))
                            .unwrap_or_default()
                    } else {
                        self.projects_view
                            .open_project
                            .as_ref()
                            .and_then(|project| {
                                crate::agents::installation_root(project, &target).map(|root| {
                                    root.strip_prefix(project)
                                        .map(|relative| relative.display().to_string())
                                        .unwrap_or_else(|_| display_path(&root))
                                })
                            })
                            .unwrap_or_default()
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_install_target(target.clone());
                this.notify_dialog(cx);
            }))
    }

    pub(super) fn delete_confirmation_modal(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let Some(confirmation) = self.delete_flow.confirmation.clone() else {
            return div();
        };
        let (title, message, location, action, success) = match &confirmation {
            DeleteConfirmation::LibrarySkills { skills } => {
                let linked_count = skills
                    .iter()
                    .filter(|(name, _)| self.model.library.is_linked_source(name))
                    .count();
                let labels = skills
                    .iter()
                    .map(|(name, _)| {
                        self.model
                            .skills
                            .iter()
                            .find(|skill| skill_storage_name(skill) == name.as_str())
                            .map(|skill| skill.record.name.clone())
                            .unwrap_or_else(|| name.clone())
                    })
                    .collect::<Vec<_>>();
                let label = labels.join(", ");
                (
                    if labels.len() == 1 {
                        t!("install.delete_confirm", name = label).to_string()
                    } else {
                        t!("install.delete_batch_confirm", count = labels.len()).to_string()
                    },
                    (if linked_count == skills.len() {
                        t!("install.unlink_hint").to_string()
                    } else if linked_count > 0 {
                        t!("install.delete_copies_hint").to_string()
                    } else {
                        t!("install.delete_files_warning").to_string()
                    })
                    .to_string(),
                    Some(
                        skills
                            .iter()
                            .map(|(_, path)| display_path(path))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    t!("common.delete").to_string(),
                    t!("install.skills_deleted").to_string(),
                )
            }
            DeleteConfirmation::ProjectSkills {
                project,
                skills,
                batch,
            } => {
                let global_scope = dirs::home_dir()
                    .as_ref()
                    .is_some_and(|home| home == project);
                let kinds = skills
                    .iter()
                    .flat_map(|skill| &skill.installations)
                    .filter(|installation| self.delete_flow.selected.contains(&installation.path))
                    .map(project::removal_kind)
                    .collect::<Vec<_>>();
                let deletes_files = kinds
                    .iter()
                    .any(|kind| matches!(kind, project::RemovalKind::SourceFiles));
                let external_sources = unique_external_sources(&kinds);
                let message = if *batch && global_scope {
                    t!("install.global_delete_hint").to_string()
                } else if *batch {
                    t!("install.project_delete_hint").to_string()
                } else if deletes_files {
                    if global_scope {
                        t!("install.global_delete_warning").to_string()
                    } else {
                        t!("install.project_delete_warning").to_string()
                    }
                } else if !external_sources.is_empty() {
                    if global_scope {
                        t!("install.global_source_kept").to_string()
                    } else {
                        t!("install.project_source_kept").to_string()
                    }
                } else {
                    if global_scope {
                        t!("install.global_library_kept").to_string()
                    } else {
                        t!("install.project_library_kept").to_string()
                    }
                };
                (
                    if *batch && global_scope {
                        t!("install.global_batch_remove_confirm").to_string()
                    } else if *batch {
                        t!("install.project_batch_remove_confirm").to_string()
                    } else if global_scope {
                        t!("install.global_remove_confirm", name = skills[0].name).to_string()
                    } else {
                        t!("install.project_remove_confirm", name = skills[0].name).to_string()
                    },
                    message.to_string(),
                    (!*batch && !external_sources.is_empty()).then(|| external_sources.join("\n")),
                    if *batch {
                        t!("common.remove").to_string()
                    } else if deletes_files {
                        t!("common.delete").to_string()
                    } else {
                        t!("common.remove").to_string()
                    },
                    if global_scope {
                        t!("install.global_removed").to_string()
                    } else {
                        t!("install.project_removed").to_string()
                    },
                )
            }
        };

        let close_button = kit::close_button(p, "close-delete-modal", true).on_click(cx.listener(
            |this, _, _, cx| {
                this.delete_flow.confirmation = None;
                this.close_dialog(cx);
            },
        ));
        let mut body = kit::modal_body()
            .id("delete-confirmation-body")
            .pt(px(4.))
            .max_h(px(520.))
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(fs_body()))
                    .line_height(relative(1.5))
                    .text_color(p.secondary)
                    .child(message),
            )
            .when_some(location, |body, location| {
                body.child(
                    kit::inset_box(p)
                        .mt(px(12.))
                        .px(px(10.))
                        .py(px(8.))
                        .font_family(mono())
                        .text_size(px(fs_small()))
                        .text_color(p.secondary)
                        .child(location),
                )
            });
        if let DeleteConfirmation::ProjectSkills { skills, batch, .. } = &confirmation {
            body = body.child(
                kit::field_label(p, t!("install.removal_title").to_string())
                    .mt(px(16.))
                    .mb(px(6.)),
            );
            for skill in skills {
                if *batch || skills.len() > 1 {
                    body = body.child(
                        div()
                            .mt(px(12.))
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .font_weight(weight_semibold())
                            .child(skill.name.clone()),
                    );
                }
                let unique_paths = unique_installation_paths(&skill.installations);
                for installation in skill
                    .installations
                    .iter()
                    .filter(|installation| unique_paths.contains(&installation.path))
                {
                    let path = installation.path.clone();
                    let checked = self.delete_flow.selected.contains(&path);
                    let Some(root) = installation.path.parent() else {
                        continue;
                    };
                    let target = display_path(root);
                    let removal_kind = project::removal_kind(installation);
                    let deletes_files = matches!(removal_kind, project::RemovalKind::SourceFiles);
                    let external_source = match removal_kind {
                        project::RemovalKind::ExternalLink { source } if *batch => {
                            Some(display_path(&source))
                        }
                        _ => None,
                    };
                    body = body.child(
                        kit::option_tile(p)
                            .id(ElementId::Name(
                                format!("delete-path-{}", path.display()).into(),
                            ))
                            .mt(px(6.))
                            .min_h(px(row_2()))
                            .px(px(12.))
                            .py(px(8.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .child(
                                Checkbox::new(ElementId::Name(
                                    format!("delete-checkbox-{}", path.display()).into(),
                                ))
                                .checked(checked),
                            )
                            .child(
                                div()
                                    .ml(px(10.))
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .text_size(px(fs_body()))
                                            .child(target)
                                            .when(*batch && deletes_files, |title| {
                                                title.child(
                                                    div()
                                                        .text_size(px(fs_caption()))
                                                        .text_color(p.danger)
                                                        .child(
                                                            t!("install.delete_files").to_string(),
                                                        ),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .mt(px(2.))
                                            .font_family(mono())
                                            .text_size(px(fs_caption()))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(display_path(&path)),
                                    )
                                    .when_some(external_source, |row, source| {
                                        row.child(
                                            div()
                                                .mt(px(2.))
                                                .font_family(mono())
                                                .text_size(px(fs_caption()))
                                                .text_color(p.muted)
                                                .child(
                                                    t!("install.original_path", path = source)
                                                        .to_string(),
                                                ),
                                        )
                                    }),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.delete_flow.selected.remove(&path) {
                                    this.delete_flow.selected.insert(path.clone());
                                }
                                this.notify_dialog(cx);
                            })),
                    );
                }
            }
        }
        let can_confirm = !self.delete_flow.busy
            && (!matches!(confirmation, DeleteConfirmation::ProjectSkills { .. })
                || !self.delete_flow.selected.is_empty());
        let cancel = kit::button(
            p,
            "cancel-delete",
            kit::ButtonKind::Ghost,
            kit::Size::Md,
            true,
        )
        .child(t!("common.cancel").to_string())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.delete_flow.confirmation = None;
                this.close_dialog(cx);
            }),
        );
        kit::modal_panel(p)
            .child(kit::modal_header(title, close_button))
            .child(body)
            .child(
                kit::modal_footer(p).child(cancel).child(
                    kit::button(
                        p,
                        "confirm-delete",
                        kit::ButtonKind::Danger,
                        kit::Size::Md,
                        can_confirm,
                    )
                    .child(action)
                    .when(can_confirm, |button| {
                        button.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if this.delete_flow.busy {
                                    return;
                                }
                                this.delete_flow.busy = true;
                                match &confirmation {
                                    DeleteConfirmation::LibrarySkills { skills } => {
                                        let mut removed = 0usize;
                                        let mut failures = Vec::new();
                                        for (name, _) in skills {
                                            match this.model.library.remove_by_storage(name) {
                                                Ok(()) => removed += 1,
                                                Err(error) => {
                                                    failures.push(this.error_message(error))
                                                }
                                            }
                                        }
                                        this.delete_flow.confirmation = None;
                                        this.close_dialog(cx);
                                        this.refresh(cx);
                                        if failures.is_empty() {
                                            this.show_notice(success.clone(), cx);
                                        } else {
                                            this.show_notice(
                                                t!(
                                                    "install.deleted_failed",
                                                    count = removed,
                                                    failed = failures.len()
                                                )
                                                .to_string(),
                                                cx,
                                            );
                                        }
                                    }
                                    DeleteConfirmation::ProjectSkills {
                                        project, skills, ..
                                    } => {
                                        let report = project::remove_project_skills(
                                            skills
                                                .iter()
                                                .flat_map(|skill| &skill.installations)
                                                .filter(|installation| {
                                                    this.delete_flow
                                                        .selected
                                                        .contains(&installation.path)
                                                }),
                                        );
                                        let message = if report.failures.is_empty() {
                                            success.to_string()
                                        } else if report.removed > 0 {
                                            crate::i18n::tn(
                                                "install.removal_partial",
                                                report.removed,
                                                &[
                                                    ("count", (report.removed).to_string()),
                                                    ("failed", (report.failures.len()).to_string()),
                                                ],
                                            )
                                        } else {
                                            crate::i18n::tn(
                                                "install.removal_failed",
                                                report.failures.len(),
                                                &[("count", (report.failures.len()).to_string())],
                                            )
                                        };
                                        if report.removed > 0 {
                                            this.projects_view
                                                .context_estimates
                                                .borrow_mut()
                                                .remove(project);
                                            if !dirs::home_dir()
                                                .as_ref()
                                                .is_some_and(|home| home == project)
                                            {
                                                this.model.library.config.touch_project(&project);
                                                let _ = this.model.library.save();
                                            }
                                        }
                                        this.delete_flow.confirmation = None;
                                        this.delete_flow.selected.clear();
                                        this.projects_view.batch_project = None;
                                        this.projects_view.batch_selected.clear();
                                        this.projects_view.batch_filter = None;
                                        this.close_dialog(cx);
                                        this.show_notice(message, cx);
                                        this.refresh(cx);
                                    }
                                }
                            }),
                        )
                    }),
                ),
            )
    }
}
