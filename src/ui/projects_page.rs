use super::*;

fn selected_project_skills(
    rows: &[EffectiveSkillRow],
    selected_names: &HashSet<String>,
) -> Vec<ProjectSkill> {
    rows.iter()
        .filter(|row| !row.direct_installations.is_empty() && selected_names.contains(&row.key))
        .map(|row| ProjectSkill {
            name: row.name.clone(),
            installations: row.direct_installations.clone(),
        })
        .collect()
}

impl KitterApp {
    fn project_skill_filter_control(&self, cx: &mut Context<Self>) -> Popover {
        let p = self.palette();
        let app = cx.entity().downgrade();
        let groups = self.model.library.groups();
        let tags = self
            .tags_flow
            .skills
            .tags()
            .iter()
            .map(|tag| {
                (
                    tag.id,
                    self.tags_flow
                        .skills
                        .path(tag.id)
                        .unwrap_or_else(|| tag.name.clone()),
                )
            })
            .collect::<Vec<_>>();
        let selected = self.projects_view.batch_filter.clone();
        let filter_tooltip = if selected.is_some() {
            t!("projects.change_filter").to_string()
        } else {
            t!("projects.filter").to_string()
        }
        .to_string();
        let all_label = t!("common.all_skills").to_string();
        let group_label = t!("common.groups").to_string();
        let tag_label = t!("common.tags").to_string();
        Popover::new("project-skill-batch-filter")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                kit::icon_trigger(
                    p,
                    "project-skill-batch-filter-trigger",
                    "icons/hash.svg",
                    filter_tooltip.clone(),
                )
                .selected_fill(p, selected.is_some())
                .tooltip(move |window, cx| Tooltip::new(filter_tooltip.clone()).build(window, cx)),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p)
                    .id("project-skill-filter-scroll")
                    .w(px(220.))
                    .max_h(px(360.))
                    .overflow_y_scroll();
                let all_app = app.clone();
                menu = menu.child(
                    kit::menu_item(
                        p,
                        "project-skill-filter-all",
                        all_label.clone(),
                        selected.is_none(),
                    )
                    .on_click(popover_cx.listener(move |_, _, _, cx| {
                        let _ = all_app.update(cx, |this, cx| {
                            this.projects_view.batch_filter = None;
                            cx.notify();
                        });
                        cx.emit(DismissEvent);
                    })),
                );
                if !groups.is_empty() {
                    menu = menu
                        .child(kit::menu_separator(p))
                        .child(kit::menu_label(p, group_label.clone()));
                }
                for group in &groups {
                    let filter = ProjectSkillFilter::Group(group.id.clone());
                    let group_app = app.clone();
                    let checked = selected.as_ref() == Some(&filter);
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(
                                format!("project-skill-filter-group-{}", group.id).into(),
                            ),
                            group.name.clone(),
                            checked,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = group_app.update(cx, |this, cx| {
                                    this.projects_view.batch_filter = Some(filter.clone());
                                    cx.notify();
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                if !tags.is_empty() {
                    menu = menu
                        .child(kit::menu_separator(p))
                        .child(kit::menu_label(p, tag_label.clone()));
                }
                for (id, name) in &tags {
                    let filter = ProjectSkillFilter::Tag(*id);
                    let tag_app = app.clone();
                    let checked = selected.as_ref() == Some(&filter);
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("project-skill-filter-tag-{id:?}").into()),
                            name.clone(),
                            checked,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = tag_app.update(cx, |this, cx| {
                                    this.projects_view.batch_filter = Some(filter.clone());
                                    cx.notify();
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            })
    }

    fn project_batch_controls(
        &self,
        open_project: &Path,
        all_rows: &[EffectiveSkillRow],
        visible_indices: Option<&[usize]>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let selected = selected_project_skills(all_rows, &self.projects_view.batch_selected);
        let selected_count = selected.len();
        let visible_rows: Box<dyn Iterator<Item = &EffectiveSkillRow>> = match visible_indices {
            Some(indices) => Box::new(indices.iter().map(|&index| &all_rows[index])),
            None => Box::new(all_rows.iter()),
        };
        let selectable_names = visible_rows
            .filter(|row| !row.direct_installations.is_empty())
            .map(|row| row.key.clone())
            .collect::<Vec<_>>();
        let all_selected = !selectable_names.is_empty()
            && selectable_names
                .iter()
                .all(|name| self.projects_view.batch_selected.contains(name));
        let project = open_project.to_path_buf();
        let global_scope = dirs::home_dir()
            .as_ref()
            .is_some_and(|home| home == open_project);
        let select_all_label = if all_selected {
            t!("projects.deselect_all").to_string()
        } else {
            t!("projects.select_visible").to_string()
        }
        .to_string();
        let remove_label = if global_scope {
            t!("projects.global_remove").to_string()
        } else {
            t!("projects.remove_from_project").to_string()
        }
        .to_string();
        let select_first_label = t!("projects.select_skills_first").to_string();
        let exit_label = t!("projects.exit_selection").to_string();
        let select_names = selectable_names.clone();
        let mut controls = div()
            .h(px(37.))
            .flex()
            .items_center()
            .gap(px(4.))
            .child(kit::count(p, selected_count).mr(px(4.)));
        if !select_names.is_empty() {
            controls = controls.child(
                div()
                    .id("project-batch-select-visible")
                    .size(px(h_sm()))
                    .rounded(px(r_md()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover_fill(p)
                    .child(
                        Checkbox::new("project-batch-select-visible-checkbox")
                            .tab_stop(false)
                            .checked(all_selected),
                    )
                    .tooltip(move |window, cx| {
                        Tooltip::new(select_all_label.clone()).build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        for name in &select_names {
                            if all_selected {
                                this.projects_view.batch_selected.remove(name);
                            } else {
                                this.projects_view.batch_selected.insert(name.clone());
                            }
                        }
                        cx.notify();
                    })),
            );
        }
        let remove = kit::icon_button(
            p,
            "project-batch-remove",
            "icons/trash.svg",
            kit::IconKind::Danger,
            kit::Size::Sm,
        )
        .tooltip(move |window, cx| {
            Tooltip::new(if selected_count > 0 {
                remove_label.clone()
            } else {
                select_first_label.clone()
            })
            .build(window, cx)
        });
        let remove = if selected_count > 0 {
            remove.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.delete_flow.selected = selected
                        .iter()
                        .flat_map(|skill| unique_installation_paths(&skill.installations))
                        .collect();
                    this.delete_flow.confirmation = Some(DeleteConfirmation::ProjectSkills {
                        project: project.clone(),
                        skills: selected.clone(),
                        batch: true,
                    });
                    this.open_delete_dialog(window, cx);
                }),
            )
        } else {
            remove.opacity(0.4).cursor_default()
        };
        controls
            .child(self.project_skill_filter_control(cx))
            .child(remove)
            .child(
                kit::icon_button(
                    p,
                    "project-batch-cancel",
                    "icons/x.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .tooltip(move |window, cx| Tooltip::new(exit_label.clone()).build(window, cx))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.projects_view.batch_project = None;
                    this.projects_view.batch_selected.clear();
                    this.projects_view.batch_filter = None;
                    cx.notify();
                })),
            )
    }

    fn project_skill_row(
        &self,
        effective: &EffectiveSkillRow,
        open_project: &Path,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let managed = effective
            .direct_installations
            .iter()
            .any(|installation| installation.managed);
        let mut location_labels = effective
            .locations
            .iter()
            .map(|location| display_effective_root(location, open_project))
            .collect::<Vec<_>>();
        if effective.built_in {
            location_labels.push(t!("projects.built_in").to_string());
        }
        let name = effective.key.clone();
        let description = effective.description.clone();
        let installed_for_delete = ProjectSkill {
            name: effective.name.clone(),
            installations: effective.direct_installations.clone(),
        };
        let can_delete = !installed_for_delete.installations.is_empty();
        let batch_mode = self.projects_view.batch_project.as_deref() == Some(open_project);
        let batch_checked = self.projects_view.batch_selected.contains(&name);
        let row_selector = format!("project-skill-{}", effective.name);
        let mut row = div()
            .id(ElementId::Name(format!("project-skill-{name}").into()))
            .debug_selector(move || row_selector)
            .w_full()
            .h(px(row_2() + 12.))
            .relative()
            .px(px(32.))
            .border_b(px(divider_w()))
            .border_color(p.border)
            .flex()
            .items_center()
            .hover_fill(p)
            .when(batch_mode && can_delete, |row| {
                row.child(
                    div()
                        .absolute()
                        .left(px(8.))
                        .top(px(0.))
                        .bottom(px(0.))
                        .flex()
                        .items_center()
                        .child(
                            Checkbox::new(ElementId::Name(
                                format!("batch-project-skill-{name}").into(),
                            ))
                            .tab_stop(false)
                            .checked(batch_checked),
                        ),
                )
            })
            .child(div().size(px(icon_md())).flex_none().child(Self::icon(
                "icons/package.svg",
                icon_md(),
                p.secondary,
            )))
            .child(
                div()
                    .ml(px(10.))
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .id(ElementId::Name(
                                        format!("project-skill-name-{name}").into(),
                                    ))
                                    .min_w_0()
                                    .truncate()
                                    .font_family(mono())
                                    .text_size(px(fs_body()))
                                    .child(effective.name.clone())
                                    .when(!description.is_empty(), |name| {
                                        let tooltip_description = description.clone();
                                        name.tooltip(move |window, cx| {
                                            let description = tooltip_description.clone();
                                            Tooltip::element(move |_, _| {
                                                div()
                                                    .w(px(320.))
                                                    .max_w(px(320.))
                                                    .font_family(mono())
                                                    .whitespace_normal()
                                                    .line_height(relative(1.45))
                                                    .text_size(px(fs_small()))
                                                    .child(description.clone())
                                            })
                                            .build(window, cx)
                                        })
                                    }),
                            )
                            .when(effective.manual_only, |title| {
                                title.child(self.manual_skill_badge().ml(px(10.)))
                            })
                            .when(managed, |title| {
                                title.child(self.managed_skill_badge().ml(px(8.)))
                            }),
                    )
                    .child(
                        div()
                            .mt(px(3.))
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .gap(px(12.))
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .text_color(p.muted)
                            .children(
                                location_labels
                                    .into_iter()
                                    .map(|label| div().flex_none().child(label)),
                            ),
                    ),
            )
            .child(
                self.effective_agent_badges(format!("project-{name}"), &effective.agents, cx)
                    .ml(px(12.)),
            );
        if batch_mode && can_delete {
            let selected_name = name.clone();
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.projects_view.batch_selected.remove(&selected_name) {
                        this.projects_view
                            .batch_selected
                            .insert(selected_name.clone());
                    }
                    cx.notify();
                }));
        } else if can_delete {
            let project_for_delete = open_project.to_path_buf();
            row = row.child(
                self.danger_icon_button(
                    ElementId::Name(format!("remove-project-{name}").into()),
                    "icons/trash.svg",
                    cx,
                )
                .ml(px(10.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.delete_flow.selected =
                            unique_installation_paths(&installed_for_delete.installations)
                                .into_iter()
                                .collect();
                        this.delete_flow.confirmation = Some(DeleteConfirmation::ProjectSkills {
                            project: project_for_delete.clone(),
                            skills: vec![installed_for_delete.clone()],
                            batch: false,
                        });
                        this.open_delete_dialog(window, cx);
                    }),
                ),
            );
        }
        row.into_any_element()
    }

    pub(super) fn projects_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let query = self
            .projects_view
            .project_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let global_root = dirs::home_dir();
        let global_skill_count = global_root
            .as_ref()
            .and_then(|path| self.context_estimate_snapshot(path, cx))
            .map(|snapshot| snapshot.view(None).rows.len());
        let projects = self
            .model
            .library
            .config
            .project_paths()
            .into_iter()
            .filter(|path| {
                let matches_query = query.is_empty()
                    || path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query)
                    || path.display().to_string().to_lowercase().contains(&query);
                matches_query
            })
            .collect::<Vec<_>>();
        let project_context_app = cx.entity().downgrade();
        let mut recent_rows = div()
            .id("projects-scroll")
            .flex_1()
            .min_h_0()
            .px(px(8.))
            .track_scroll(&self.projects_view.list_scroll)
            .overflow_y_scroll();
        if projects.is_empty() {
            recent_rows = recent_rows.child(
                div()
                    .mt(px(30.))
                    .px(px(16.))
                    .text_size(px(fs_body()))
                    .text_color(p.muted)
                    .text_center()
                    .child(if self.model.library.config.recent_projects.is_empty() {
                        t!("projects.empty_title").to_string()
                    } else {
                        t!("projects.search_empty").to_string()
                    }),
            );
        }
        for path in projects {
            let selected_path = path.clone();
            let selected = !self.projects_view.global_project_view
                && self.projects_view.open_project.as_ref() == Some(&path);
            let reveal_path = path.clone();
            let remove_app = project_context_app.clone();
            let remove_path = path.clone();
            let reveal_label = if cfg!(target_os = "macos") {
                t!("common.show_in_finder").to_string()
            } else {
                t!("common.show_in_folder").to_string()
            }
            .to_string();
            let remove_label = t!("projects.remove_project").to_string();
            let remove_menu_color = p.danger;
            recent_rows = recent_rows.child(
                kit::list_row(
                    p,
                    ElementId::Name(format!("project-{}", path.display()).into()),
                    selected,
                )
                .min_h(px(row_2()))
                .my(px(1.))
                .child(Self::icon("icons/folder.svg", icon_md(), p.secondary))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .py(px(6.))
                        .child(
                            div()
                                .font_family(mono())
                                .text_size(px(fs_body()))
                                .font_weight(weight_medium())
                                .truncate()
                                .child(
                                    path.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                ),
                        )
                        .child(
                            div()
                                .font_family(mono())
                                .text_size(px(fs_small()))
                                .text_color(p.muted)
                                .truncate()
                                .child(display_path(&path)),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.projects_view.open_project.as_ref() != Some(&selected_path) {
                        this.projects_view.batch_project = None;
                        this.projects_view.batch_selected.clear();
                        this.projects_view.batch_filter = None;
                    }
                    this.projects_view.open_project = Some(selected_path.clone());
                    this.projects_view.global_project_view = false;
                    this.projects_view.selected_project_agent = None;
                    this.projects_view.project_agents_expanded = false;
                    cx.notify();
                }))
                .context_menu(move |menu, _, _| {
                    let reveal_path = reveal_path.clone();
                    let remove_app = remove_app.clone();
                    let remove_path = remove_path.clone();
                    let remove_menu_item =
                        Self::danger_menu_item(remove_label.clone(), remove_menu_color);
                    menu.min_w(px(160.))
                        .item(
                            PopupMenuItem::new(reveal_label.clone())
                                .icon(
                                    Icon::default().path(KitterApp::icon_path("icons/folder.svg")),
                                )
                                .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
                        )
                        .item(remove_menu_item.on_click(move |_, window, cx| {
                            let _ = remove_app.update(cx, |this, cx| {
                                this.model.library.config.remove_project(&remove_path);
                                if this.projects_view.open_project.as_ref() == Some(&remove_path) {
                                    this.projects_view.open_project = None;
                                    this.projects_view.global_project_view = true;
                                }
                                if this.projects_view.batch_project.as_ref() == Some(&remove_path) {
                                    this.projects_view.batch_project = None;
                                    this.projects_view.batch_selected.clear();
                                    this.projects_view.batch_filter = None;
                                }
                                let _ = this.model.library.save();
                                this.sync_project_select(window, cx);
                                cx.notify();
                            });
                        }))
                }),
            );
        }
        let global_selected = self.projects_view.global_project_view;
        let global_row = kit::list_row(p, "global-project-skills", global_selected)
            .w_auto()
            .min_h(px(row_2()))
            .mx(px(8.))
            .mb(px(6.))
            .child(Self::icon("icons/house.svg", icon_md(), p.secondary))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .font_family(mono())
                            .text_size(px(fs_body()))
                            .font_weight(weight_medium())
                            .child(t!("projects.global").to_string()),
                    )
                    .child(
                        div()
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .text_color(p.muted)
                            .truncate()
                            .child(match global_skill_count {
                                Some(count) => crate::i18n::tn(
                                    "projects.user_level_skills",
                                    count,
                                    &[("count", (count).to_string())],
                                ),
                                None => t!("common.scanning").to_string().into(),
                            }),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.projects_view.batch_project = None;
                this.projects_view.batch_selected.clear();
                this.projects_view.batch_filter = None;
                this.projects_view.global_project_view = true;
                this.projects_view.selected_project_agent = None;
                this.projects_view.project_agents_expanded = false;
                cx.notify();
            }));
        let recent_panel = kit::pane(p)
            .w_full()
            .child(
                self.panel_header(
                    "projects-panel-window-drag",
                    t!("common.projects").to_string(),
                    cx,
                )
                .child(
                    self.labeled_icon_button(
                        "open-project-folder",
                        "icons/plus.svg",
                        t!("projects.folder_picker").to_string(),
                        cx,
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.browse_project(window, cx)),
                    ),
                ),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .child(kit::search_input(p, &self.projects_view.project_search).small()),
            )
            .child(global_row)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(recent_rows)
                    .child(kit::scrollbar(
                        "projects-scrollbar",
                        &self.projects_view.list_scroll,
                    )),
            );

        let selected_root = if self.projects_view.global_project_view {
            global_root.as_ref()
        } else {
            self.projects_view.open_project.as_ref()
        };
        let detail = if let Some(open_project) = selected_root {
            let is_global = self.projects_view.global_project_view;
            let snapshot = self.context_estimate_snapshot(open_project, cx);
            let loading = snapshot.is_none();
            let snapshot = snapshot.unwrap_or_default();
            let view = snapshot.view(self.projects_view.selected_project_agent.clone());
            let context_panel = if loading {
                div()
                    .h(px(86.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_size(px(fs_body()))
                    .text_color(p.muted)
                    .child(self.shell.spinner_accent.clone())
                    .child(t!("projects.scanning_project").to_string())
            } else {
                self.context_estimate_panel(open_project, &snapshot.estimates, cx)
            };
            let all_effective_rows = view.rows;
            let batch_mode = self.projects_view.batch_project.as_deref() == Some(open_project);
            let can_batch = batch_mode || view.can_batch;
            let visible_indices: Option<Arc<[usize]>> = if batch_mode {
                self.projects_view.batch_filter.as_ref().map(|filter| {
                    let keys = self
                        .model
                        .skills
                        .iter()
                        .filter(|skill| match filter {
                            ProjectSkillFilter::Group(group) => {
                                skill.record.group_id.as_ref() == Some(group)
                            }
                            ProjectSkillFilter::Tag(tag) => self
                                .tags_flow
                                .skills
                                .matches_filter(skill_storage_name(skill), *tag),
                        })
                        .map(skill_storage_name)
                        .collect::<HashSet<_>>();
                    all_effective_rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            row.library_keys
                                .iter()
                                .any(|key| keys.contains(key.as_str()))
                                .then_some(index)
                        })
                        .collect::<Vec<_>>()
                        .into()
                })
            } else {
                None
            };
            let row_count = visible_indices
                .as_ref()
                .map_or(all_effective_rows.len(), |indices| indices.len());
            let mut skills = div()
                .id("project-skills-scroll")
                .debug_selector(|| "project-skills-scroll".into())
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .px(px(0.))
                .pt(px(14.))
                .pb(px(28.));
            if row_count == 0 && !loading {
                let selected_agent = self
                    .projects_view
                    .selected_project_agent
                    .as_ref()
                    .and_then(|agent| {
                        crate::agent_plugins::AgentRegistry::global().agent(agent.as_str())
                    })
                    .map(|agent| agent.name.as_str())
                    .unwrap_or_default();
                skills = skills.child(
                    kit::empty_state(
                        p,
                        "icons/package.svg",
                        if self.projects_view.selected_project_agent.is_some() {
                            t!("projects.agent_empty", agent = selected_agent).to_string()
                        } else if is_global {
                            t!("projects.user_skills_empty").to_string()
                        } else {
                            t!("projects.effective_empty").to_string()
                        },
                    )
                    .mt(px(56.)),
                );
            }
            if row_count > 0 {
                let rows = Arc::clone(&all_effective_rows);
                let indices = visible_indices.clone();
                let project = open_project.clone();
                skills = skills.child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(
                            uniform_list(
                                "project-skills-virtual-list",
                                row_count,
                                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|index| {
                                            let index = indices
                                                .as_ref()
                                                .map_or(index, |indices| indices[index]);
                                            this.project_skill_row(&rows[index], &project, cx)
                                        })
                                        .collect()
                                }),
                            )
                            .debug_selector(|| "project-skills-virtual-list".into())
                            .track_scroll(&self.projects_view.skills_scroll)
                            .flex_1()
                            .min_h_0(),
                        )
                        .child(kit::scrollbar(
                            "project-skills-scrollbar",
                            &self.projects_view.skills_scroll,
                        )),
                );
            }

            let mut tabs = self.project_skills_tabs(cx).child(div().flex_1());
            if can_batch && self.projects_view.project_skills_tab == ProjectSkillsTab::Skills {
                if batch_mode {
                    tabs = tabs.child(self.project_batch_controls(
                        open_project,
                        &all_effective_rows,
                        visible_indices.as_deref(),
                        cx,
                    ));
                } else {
                    let batch_project_path = open_project.clone();
                    let batch_label = t!("projects.select_multiple").to_string();
                    tabs = tabs.child(
                        div().h(px(37.)).flex().items_center().child(
                            kit::icon_button(
                                p,
                                "project-batch-toggle",
                                "icons/check.svg",
                                kit::IconKind::Ghost,
                                kit::Size::Sm,
                            )
                            .tooltip(move |window, cx| {
                                Tooltip::new(batch_label.clone()).build(window, cx)
                            })
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.projects_view.batch_project =
                                        Some(batch_project_path.clone());
                                    cx.notify();
                                },
                            )),
                        ),
                    );
                }
            }
            let skills = match self.projects_view.project_skills_tab {
                ProjectSkillsTab::Skills => skills,
                ProjectSkillsTab::Plugins => self.effective_plugins_list(&view.plugins, cx),
                ProjectSkillsTab::Presets => self.project_presets_list(open_project, cx),
            };
            div()
                .relative()
                .min_w_0()
                .flex_1()
                .h_full()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px(px(24.))
                        .pt(px(24.))
                        .pb(px(16.))
                        .border_b(px(divider_w()))
                        .border_color(p.border)
                        .child(kit::detail_header(
                            p,
                            if is_global {
                                "icons/house.svg"
                            } else {
                                "icons/folder.svg"
                            },
                            div().font_family(mono()).child(self.selectable_text(
                                "project-detail-name",
                                300,
                                if is_global {
                                    t!("projects.global").to_string()
                                } else {
                                    open_project
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned()
                                },
                                window,
                                cx,
                            )),
                            div().font_family(mono()).child(self.selectable_text(
                                "project-detail-path",
                                301,
                                display_path(open_project),
                                window,
                                cx,
                            )),
                            div(),
                        )),
                )
                .child(
                    div()
                        .px(px(28.))
                        .pt(px(10.))
                        .pb(px(12.))
                        .flex_none()
                        .border_b(px(divider_w()))
                        .border_color(p.border)
                        .child(context_panel),
                )
                .child(tabs)
                .child(skills)
                .child(self.window_drag_strip("project-detail-window-drag", 24., cx))
        } else {
            let first = self.model.library.config.recent_projects.is_empty();
            kit::empty_page(
                p,
                "icons/folder.svg",
                if first {
                    t!("projects.empty_action").to_string()
                } else {
                    t!("projects.open_folder").to_string()
                },
            )
            .child(
                div()
                    .mt(px(8.))
                    .max_w(px(430.))
                    .text_center()
                    .text_size(px(fs_body()))
                    .line_height(relative(1.55))
                    .text_color(p.muted)
                    .child(t!("projects.empty_hint").to_string()),
            )
            .child(
                kit::button(
                    p,
                    "empty-open-project",
                    kit::ButtonKind::Secondary,
                    kit::Size::Md,
                    true,
                )
                .mt(px(16.))
                .child(Self::icon("icons/folder.svg", icon_md(), p.secondary))
                .child(t!("projects.folder_picker").to_string())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.browse_project(window, cx)),
                ),
            )
            .child(self.window_drag_strip("empty-project-detail-window-drag", 24., cx))
        };
        layout::content(
            &self.shell.content_layout,
            self.content_pane_width(window, cx),
            recent_panel,
            detail,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use crate::{InstallTarget, ProjectSkillInstallation};

    use super::{EffectiveSkillRow, selected_project_skills};

    fn row(name: &str, paths: &[&str]) -> EffectiveSkillRow {
        EffectiveSkillRow {
            key: name.into(),
            name: name.into(),
            description: String::new(),
            locations: Vec::new(),
            built_in: false,
            agents: Vec::new(),
            manual_only: false,
            library_keys: Vec::new(),
            direct_installations: paths
                .iter()
                .map(|path| ProjectSkillInstallation {
                    target: InstallTarget::universal(),
                    path: PathBuf::from(path),
                    managed: true,
                })
                .collect(),
        }
    }

    #[test]
    fn selecting_one_same_named_skill_does_not_select_the_other_source() {
        let mut first = row("same-name", &["/first"]);
        first.key = "file:first".into();
        let mut second = row("same-name", &["/second"]);
        second.key = "file:second".into();
        let selected = std::collections::HashSet::from([first.key.clone()]);
        let result = selected_project_skills(&[first, second], &selected);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].installations[0].path,
            std::path::PathBuf::from("/first")
        );
    }

    #[test]
    fn batch_removal_uses_only_the_selected_rows_direct_installations() {
        let rows = vec![
            row("selected", &["/project/.agents/skills/selected"]),
            row("shadowed", &["/project/.claude/skills/shadowed"]),
            row("external-only", &[]),
        ];
        let selected = HashSet::from(["selected".to_string(), "external-only".to_string()]);

        let targets = selected_project_skills(&rows, &selected);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "selected");
        assert_eq!(targets[0].installations.len(), 1);
        assert_eq!(
            targets[0].installations[0].path,
            PathBuf::from("/project/.agents/skills/selected")
        );
    }
}
