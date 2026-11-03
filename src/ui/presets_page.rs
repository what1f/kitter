use super::*;
use crate::preset::Preset;

impl KitterApp {
    pub(super) fn presets_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let presets = self.presets_view.store.presets();
        let mut rows = div()
            .id("presets-scroll")
            .debug_selector(|| "presets-scroll".into())
            .flex_1()
            .min_h_0()
            .px(px(8.))
            .py(px(8.))
            .track_scroll(&self.presets_view.list_scroll)
            .overflow_y_scroll();
        for preset in presets {
            rows = rows.child(self.preset_list_row(preset, cx));
        }
        let list = kit::pane(p)
            .w_full()
            .child(
                self.panel_header(
                    "presets-panel-window-drag",
                    t!("presets.title").to_string(),
                    cx,
                )
                .child(
                    self.labeled_icon_button(
                        "add-preset",
                        "icons/plus.svg",
                        t!("presets.new").to_string(),
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_preset_name_dialog(PresetEdit::Create, window, cx);
                    })),
                ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(rows)
                    .child(kit::scrollbar(
                        "presets-scrollbar",
                        &self.presets_view.list_scroll,
                    )),
            );
        layout::content(
            &self.shell.content_layout,
            self.content_pane_width(window, cx),
            list,
            self.preset_detail(window, cx),
        )
    }

    fn preset_list_row(&self, preset: &Preset, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let id = preset.id.clone();
        let selected = self.presets_view.selected.as_deref() == Some(id.as_str());
        let context_app = cx.entity().downgrade();
        let rename_label = t!("common.rename").to_string();
        let install_label = t!("presets.install").to_string();
        let delete_label = t!("common.delete").to_string();
        let delete_menu_color = p.danger;
        let click_id = id.clone();
        let context_id = id.clone();
        kit::list_row(p, ElementId::Name(format!("preset-{id}").into()), selected)
            .my(px(1.))
            .child(Self::icon("icons/layers.svg", icon_md(), p.secondary))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .font_family(mono())
                    .text_size(px(fs_body()))
                    .font_weight(weight_medium())
                    .truncate()
                    .child(preset.name.clone()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    this.select_preset(context_id.clone(), cx);
                }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_preset(click_id.clone(), cx);
            }))
            .context_menu(move |menu, _, _| {
                let rename_app = context_app.clone();
                let install_app = context_app.clone();
                let delete_app = context_app.clone();
                let rename_id = id.clone();
                let install_id = id.clone();
                let delete_id = id.clone();
                menu.min_w(px(190.))
                    .item(
                        PopupMenuItem::new(rename_label.clone())
                            .icon(Icon::default().path(KitterApp::icon_path("icons/pencil.svg")))
                            .on_click(move |_, window, cx| {
                                let _ = rename_app.update(cx, |this, cx| {
                                    this.open_preset_name_dialog(
                                        PresetEdit::Rename(rename_id.clone()),
                                        window,
                                        cx,
                                    );
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new(install_label.clone())
                            .icon(Icon::default().path(KitterApp::icon_path("icons/download.svg")))
                            .on_click(move |_, window, cx| {
                                let _ = install_app.update(cx, |this, cx| {
                                    this.open_preset_install_dialog(
                                        install_id.clone(),
                                        None,
                                        window,
                                        cx,
                                    );
                                });
                            }),
                    )
                    .item(
                        Self::danger_menu_item(delete_label.clone(), delete_menu_color).on_click(
                            move |_, _, cx| {
                                let _ = delete_app.update(cx, |this, cx| {
                                    this.request_preset_delete(delete_id.clone(), cx);
                                });
                            },
                        ),
                    )
            })
            .into_any_element()
    }

    fn preset_detail(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let Some(preset) = self.selected_preset().cloned() else {
            return kit::empty_page(p, "icons/layers.svg", t!("presets.empty").to_string())
                .child(
                    kit::button(
                        p,
                        "empty-new-preset",
                        kit::ButtonKind::Secondary,
                        kit::Size::Md,
                        true,
                    )
                    .mt(px(16.))
                    .child(Self::icon("icons/plus.svg", icon_md(), p.secondary))
                    .child(t!("presets.new").to_string())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_preset_name_dialog(PresetEdit::Create, window, cx);
                    })),
                )
                .child(self.window_drag_strip("empty-preset-detail-window-drag", 24., cx));
        };
        let delete_id = preset.id.clone();
        let install_id = preset.id.clone();
        let actions = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                self.danger_icon_button("delete-preset", "icons/trash.svg", cx)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.request_preset_delete(delete_id.clone(), cx);
                        }),
                    ),
            )
            .child(
                self.labeled_icon_button(
                    "install-preset",
                    "icons/download.svg",
                    t!("presets.install").to_string(),
                    cx,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.open_preset_install_dialog(install_id.clone(), None, window, cx);
                    }),
                ),
            );
        let header = div()
            .px(px(24.))
            .pt(px(24.))
            .pb(px(16.))
            .child(kit::detail_header(
                p,
                "icons/layers.svg",
                div()
                    .min_w_0()
                    .font_family(mono())
                    .truncate()
                    .child(preset.name.clone()),
                div(),
                actions,
            ));
        let tabs = kit::tab_bar(p)
            .child(self.preset_tab_button(
                "preset-tab-skills",
                t!("common.skills").to_string(),
                PresetTab::Skills,
                cx,
            ))
            .child(self.preset_tab_button(
                "preset-tab-agents-md",
                t!("presets.instructions").to_string(),
                PresetTab::AgentsMd,
                cx,
            ))
            .child(self.preset_tab_button(
                "preset-tab-projects",
                t!("common.projects").to_string(),
                PresetTab::Projects,
                cx,
            ));
        let body = match self.presets_view.tab {
            PresetTab::Skills => self.preset_skills_tab(&preset, cx),
            PresetTab::AgentsMd => self.preset_agents_md_tab(&preset, window, cx),
            PresetTab::Projects => self.preset_projects_tab(&preset, cx),
        };
        div()
            .relative()
            .min_w_0()
            .flex_1()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(tabs)
            .child(body)
            .child(self.window_drag_strip("preset-detail-window-drag", 24., cx))
    }

    fn preset_tab_button(
        &self,
        id: &'static str,
        label: String,
        tab: PresetTab,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let active = self.presets_view.tab == tab;
        kit::tab(p, id, active)
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.presets_view.tab = tab;
                cx.notify();
            }))
    }

    fn preset_skills_tab(&self, preset: &Preset, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let mut card = kit::card(p);
        let mut index = 0;
        for storage_name in &preset.skills {
            let Some(skill) = self
                .model
                .skills
                .iter()
                .find(|skill| skill_storage_name(skill) == storage_name)
            else {
                continue;
            };
            let remove_id = preset.id.clone();
            let remove_name = storage_name.clone();
            card = card.child(
                kit::card_row(p, index == 0)
                    .h(px(56.))
                    .child(Self::icon(
                        if skill.record.origin.is_builtin() {
                            "icons/crown.svg"
                        } else {
                            "icons/package.svg"
                        },
                        icon_md(),
                        p.secondary,
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(mono())
                                    .text_size(px(fs_body()))
                                    .truncate()
                                    .child(skill.record.name.clone()),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .font_family(mono())
                                    .text_size(px(fs_small()))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(self.source_label(&skill.record.origin)),
                            ),
                    )
                    .child(
                        self.labeled_icon_button(
                            ElementId::Name(format!("remove-preset-skill-{storage_name}").into()),
                            "icons/x.svg",
                            t!("common.remove").to_string(),
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.remove_preset_skill(&remove_id, &remove_name, cx);
                            }),
                        ),
                    ),
            );
            index += 1;
        }
        if index == 0 {
            return div().flex_1().pt(px(56.)).child(
                kit::empty_state(
                    p,
                    "icons/package.svg",
                    t!("presets.skills_empty").to_string(),
                )
                .child(
                    kit::button(
                        p,
                        "add-preset-skill",
                        kit::ButtonKind::Secondary,
                        kit::Size::Md,
                        true,
                    )
                    .mt(px(6.))
                    .child(Self::icon("icons/plus.svg", icon_md(), p.secondary))
                    .child(t!("common.add_skill").to_string())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_preset_skills_dialog(window, cx);
                    })),
                ),
            );
        }
        div().flex_1().min_h_0().child(
            div()
                .id("preset-skills-scroll")
                .debug_selector(|| "preset-skills-scroll".into())
                .size_full()
                .px(px(24.))
                .pt(px(16.))
                .pb(px(24.))
                .overflow_y_scroll()
                .child(
                    card.child(
                        kit::card_add_row(p, "add-preset-skill")
                            .debug_selector(|| "add-preset-skill".into())
                            .child(Self::icon("icons/plus.svg", icon_md(), p.secondary))
                            .child(t!("common.add_skill").to_string())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_preset_skills_dialog(window, cx);
                            })),
                    ),
                ),
        )
    }

    fn preset_sync_all_button(
        &self,
        preset: &Preset,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let p = self.palette();
        let pending = self
            .presets_view
            .store
            .installations(&preset.id)
            .map(|installation| installation.project.clone())
            .filter(|project| self.preset_needs_sync(&preset.id, project))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return None;
        }
        let sync_id = preset.id.clone();
        Some(
            kit::button(
                p,
                "sync-all-preset-projects",
                kit::ButtonKind::Secondary,
                kit::Size::Sm,
                true,
            )
            .child(Self::icon("icons/rotate-cw.svg", icon_sm(), p.secondary))
            .child(t!("presets.sync_all").to_string())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.sync_preset(sync_id.clone(), pending.clone(), cx);
            })),
        )
    }

    fn preset_agents_md_tab(
        &self,
        preset: &Preset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let files = self.preset_files(&preset.id);
        if !files.contains(&self.presets_view.selected_file) {
            // The selected file was removed elsewhere, e.g. by the CLI.
            let app = cx.entity().downgrade();
            window.defer(cx, move |_, cx| {
                let _ = app.update(cx, |this, cx| {
                    this.presets_view.selected_file = PathBuf::from("AGENTS.md");
                    cx.notify();
                });
            });
        }
        self.load_preset_agents_md(&preset.id, window, cx);
        let new_file_id = preset.id.clone();
        let mut tree = div()
            .id("preset-files-tree")
            .size_full()
            .py(px(8.))
            .track_scroll(&self.presets_view.tree_scroll)
            .overflow_y_scroll();
        for entry in file_tree::rows(
            p,
            "preset-",
            &files,
            Some(Path::new("AGENTS.md")),
            &self.presets_view.selected_file,
            &self.presets_view.collapsed_dirs,
        ) {
            let path = entry.path;
            if entry.directory {
                tree = tree.child(entry.row.on_click(cx.listener(move |this, _, _, cx| {
                    let collapsed = &mut this.presets_view.collapsed_dirs;
                    if !collapsed.remove(&path) {
                        collapsed.insert(path.clone());
                    }
                    cx.notify();
                })));
                continue;
            }
            let select_path = path.clone();
            let row = entry.row.on_click(cx.listener(move |this, _, _, cx| {
                this.select_preset_file(select_path.clone(), cx);
            }));
            if path == Path::new("AGENTS.md") {
                tree = tree.child(row);
                continue;
            }
            let app = cx.entity().downgrade();
            let id = preset.id.clone();
            let danger = p.danger;
            tree = tree.child(row.context_menu(move |menu, _, _| {
                let (rename_app, rename_id, rename_path) = (app.clone(), id.clone(), path.clone());
                let (delete_app, delete_id, delete_path) = (app.clone(), id.clone(), path.clone());
                menu.item(
                    PopupMenuItem::new(t!("common.rename").to_string())
                        .icon(Icon::default().path(KitterApp::icon_path("icons/pencil.svg")))
                        .on_click(move |_, window, cx| {
                            let _ = rename_app.update(cx, |this, cx| {
                                this.open_preset_name_dialog(
                                    PresetEdit::RenameFile(rename_id.clone(), rename_path.clone()),
                                    window,
                                    cx,
                                );
                            });
                        }),
                )
                .item(
                    Self::danger_menu_item(t!("common.delete").to_string(), danger).on_click(
                        move |_, _, cx| {
                            let _ = delete_app.update(cx, |this, cx| {
                                this.request_preset_file_delete(
                                    delete_id.clone(),
                                    delete_path.clone(),
                                    cx,
                                );
                            });
                        },
                    ),
                )
            }));
        }
        let drop_id = preset.id.clone();
        let tree_panel = kit::pane(p)
            .id("preset-files-panel")
            .relative()
            .w(px(220.))
            .drag_over::<ExternalPaths>(move |panel, _, _, _| panel.drop_fill(p))
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                this.add_dropped_preset_files(drop_id.clone(), paths.paths(), cx);
            }))
            .child(
                div()
                    .h(px(40.))
                    .flex_none()
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(
                        kit::icon_button(
                            p,
                            "preset-new-file",
                            "icons/plus.svg",
                            kit::IconKind::Ghost,
                            kit::Size::Sm,
                        )
                        .tooltip(|window, cx| {
                            Tooltip::new(t!("presets.new_file").to_string()).build(window, cx)
                        })
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_preset_name_dialog(
                                    PresetEdit::NewFile(new_file_id.clone()),
                                    window,
                                    cx,
                                );
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(tree)
                    .child(kit::scrollbar(
                        "preset-files-scrollbar",
                        &self.presets_view.tree_scroll,
                    )),
            );
        let editor = div()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(kit::file_bar(p).child(self.presets_view.selected_file.display().to_string()))
            .child(
                div().flex_1().min_h_0().px(px(8.)).py(px(8.)).child(
                    Editor::new(&self.presets_view.agents_md)
                        .h_full()
                        .bordered(false)
                        .font_family(mono())
                        .text_size(px(fs_body())),
                ),
            );
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(tree_panel)
            .child(editor)
    }

    fn preset_projects_tab(&self, preset: &Preset, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let home = dirs::home_dir();
        let projects = self
            .presets_view
            .store
            .installations(&preset.id)
            .map(|installation| installation.project.clone())
            .collect::<Vec<_>>();
        if projects.is_empty() {
            return div().flex_1().pt(px(56.)).child(kit::empty_state(
                p,
                "icons/folder.svg",
                t!("skills.installation_empty").to_string(),
            ));
        }
        let list = div()
            .id("preset-projects-scroll")
            .debug_selector(|| "preset-projects-scroll".into())
            .size_full()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(24.))
            .overflow_y_scroll()
            .children(
                self.preset_sync_all_button(preset, cx)
                    .map(|button| div().flex().justify_end().pb(px(8.)).child(button)),
            );
        let mut card = kit::card(p);
        for (index, project) in projects.into_iter().enumerate() {
            let global = home.as_ref() == Some(&project);
            let name = if global {
                t!("common.global").to_string()
            } else {
                project
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            card = card.child(
                kit::card_row(p, index == 0)
                    .h(px(56.))
                    .child(Self::icon(
                        if global {
                            "icons/house.svg"
                        } else {
                            "icons/folder.svg"
                        },
                        icon_md(),
                        p.secondary,
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(mono())
                                    .text_size(px(fs_body()))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .font_family(mono())
                                    .text_size(px(fs_small()))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(display_path(&project)),
                            ),
                    )
                    .child(self.preset_project_actions(
                        &preset.id,
                        &project,
                        format!("preset-project-{index}"),
                        cx,
                    )),
            );
        }
        div().flex_1().min_h_0().child(list.child(card))
    }

    /// Sync (when needed) and remove buttons for one preset installation.
    pub(super) fn preset_project_actions(
        &self,
        id: &str,
        project: &Path,
        key: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let sync_id = id.to_string();
        let sync_project = project.to_path_buf();
        let remove_id = id.to_string();
        let remove_project = project.to_path_buf();
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .when(self.preset_needs_sync(id, project), |actions| {
                actions.child(
                    self.labeled_icon_button(
                        ElementId::Name(format!("sync-{key}").into()),
                        "icons/rotate-cw.svg",
                        t!("presets.sync").to_string(),
                        cx,
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.sync_preset(sync_id.clone(), vec![sync_project.clone()], cx);
                        }),
                    ),
                )
            })
            .child(
                self.danger_icon_button(
                    ElementId::Name(format!("remove-{key}").into()),
                    "icons/trash.svg",
                    cx,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.request_preset_uninstall(
                            remove_id.clone(),
                            remove_project.clone(),
                            cx,
                        );
                    }),
                ),
            )
    }

    /// The Presets tab of a project's detail.
    pub(super) fn project_presets_list(
        &self,
        project: &Path,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let installed = self
            .presets_view
            .store
            .project_installations(project)
            .map(|installation| installation.preset.clone())
            .collect::<Vec<_>>();
        let mut list = div()
            .id("project-presets-scroll")
            .debug_selector(|| "project-presets-scroll".into())
            .flex_1()
            .min_h_0()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(28.))
            .overflow_y_scroll();
        let mut card = kit::card(p);
        for (index, id) in installed.iter().enumerate() {
            let Ok(preset) = self.presets_view.store.preset(id) else {
                continue;
            };
            let open_id = id.clone();
            card = card.child(
                kit::card_row(p, index == 0)
                    .h(px(row_2()))
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("open-project-preset-{index}").into(),
                            ))
                            .debug_selector(|| format!("open-project-preset-{index}"))
                            .min_w_0()
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .cursor_pointer()
                            .child(Self::icon("icons/layers.svg", icon_md(), p.secondary))
                            .child(
                                div()
                                    .min_w_0()
                                    .font_family(mono())
                                    .text_size(px(fs_body()))
                                    .truncate()
                                    .child(preset.name.clone()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.presets_view.selected = Some(open_id.clone());
                                this.shell.page = Page::Presets;
                                cx.notify();
                            })),
                    )
                    .child(self.preset_project_actions(
                        id,
                        project,
                        format!("project-preset-{index}"),
                        cx,
                    )),
            );
        }
        if !installed.is_empty() {
            list = list.child(card.mb(px(8.)));
        }
        let choices = self
            .presets_view
            .store
            .presets()
            .iter()
            .filter(|preset| !installed.contains(&preset.id))
            .map(|preset| (preset.id.clone(), preset.name.clone()))
            .collect::<Vec<_>>();
        if !choices.is_empty() {
            let app = cx.entity().downgrade();
            let project = project.to_path_buf();
            list = list.child(
                Popover::new("install-project-preset-menu")
                    .appearance(false)
                    .anchor(Anchor::TopLeft)
                    .trigger(
                        kit::quiet_trigger(p, "install-project-preset")
                            .child(Self::icon("icons/plus.svg", icon_md(), p.secondary))
                            .child(t!("presets.install_preset").to_string()),
                    )
                    .content(move |_, _, popover_cx| {
                        let mut menu = kit::menu(p).w(px(260.));
                        for (index, (id, name)) in choices.iter().cloned().enumerate() {
                            let app = app.clone();
                            let project = project.clone();
                            menu = menu.child(
                                kit::menu_action(
                                    p,
                                    ElementId::Name(
                                        format!("install-project-preset-option-{index}").into(),
                                    ),
                                    "icons/layers.svg",
                                    name,
                                )
                                .on_click(popover_cx.listener(
                                    move |_, _, window, cx| {
                                        cx.emit(DismissEvent);
                                        let _ = app.update(cx, |this, cx| {
                                            this.open_preset_install_dialog(
                                                id.clone(),
                                                Some(project.clone()),
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                )),
                            );
                        }
                        menu
                    }),
            );
        }
        list
    }

    pub(super) fn preset_name_modal(&self, cx: &mut Context<Self>) -> Div {
        let (title, confirm) = match &self.presets_view.edit {
            Some(PresetEdit::Create) => (t!("presets.new"), t!("presets.create")),
            Some(PresetEdit::NewFile(_)) => (t!("presets.new_file"), t!("presets.create")),
            _ => (t!("common.rename"), t!("presets.save")),
        };
        let (title, confirm) = (title.to_string(), confirm.to_string());
        let p = self.palette();
        let body = kit::form()
            .gap(px(8.))
            .child(kit::input(p, &self.presets_view.name_input))
            .when_some(self.presets_view.error.clone(), |body, error| {
                body.child(kit::field_error(p, error))
            });
        self.action_dialog(
            "preset-name",
            title,
            body,
            confirm,
            false,
            true,
            t!("common.cancel").to_string(),
            |this, cx| this.close_dialog(cx),
            |this, cx| this.commit_preset_name(cx),
            cx,
        )
    }

    pub(super) fn preset_skills_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let query = self
            .presets_view
            .skill_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let included = self
            .selected_preset()
            .map(|preset| preset.skills.clone())
            .unwrap_or_default();
        let mut rows = div()
            .id("preset-skill-choices")
            .debug_selector(|| "preset-skill-choices".into())
            .mt(px(8.))
            .h(px(320.))
            .overflow_y_scroll();
        for skill in &self.model.skills {
            let storage_name = skill_storage_name(skill).to_string();
            if included.contains(&storage_name) || !self.skill_matches_query(skill, &query) {
                continue;
            }
            let picked = self.presets_view.picked_skills.contains(&storage_name);
            rows = rows.child(
                kit::list_row(
                    p,
                    ElementId::Name(format!("pick-preset-skill-{storage_name}").into()),
                    false,
                )
                .min_h(px(row_2()))
                .gap(px(10.))
                .child(
                    Checkbox::new(ElementId::Name(
                        format!("pick-preset-skill-check-{storage_name}").into(),
                    ))
                    .tab_stop(false)
                    .checked(picked),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .font_family(mono())
                                .text_size(px(fs_body()))
                                .truncate()
                                .child(skill.record.name.clone()),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .font_family(mono())
                                .text_size(px(fs_small()))
                                .text_color(p.muted)
                                .truncate()
                                .child(self.source_label(&skill.record.origin)),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.presets_view.picked_skills.remove(&storage_name) {
                        this.presets_view.picked_skills.insert(storage_name.clone());
                    }
                    this.notify_dialog(cx);
                })),
            );
        }
        let count = self.presets_view.picked_skills.len();
        let body = div()
            .child(kit::search_input(p, &self.presets_view.skill_search).small())
            .child(rows);
        self.action_dialog(
            "preset-skills",
            t!("common.add_skill").to_string(),
            body,
            t!("common.add_count", count = count).to_string(),
            false,
            count > 0,
            t!("common.cancel").to_string(),
            |this, cx| this.close_dialog(cx),
            |this, cx| this.add_picked_preset_skills(cx),
            cx,
        )
    }

    pub(super) fn preset_confirm_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let Some(confirmation) = self.presets_view.confirmation.clone() else {
            return div();
        };
        let name = |id: &str| {
            self.presets_view
                .store
                .preset(id)
                .map(|preset| preset.name.clone())
                .unwrap_or_default()
        };
        let message = |text: String| {
            div()
                .text_size(px(fs_body()))
                .line_height(relative(1.5))
                .child(text)
        };
        let hint = |text: String| {
            div()
                .mt(px(6.))
                .text_size(px(fs_body()))
                .line_height(relative(1.5))
                .text_color(p.secondary)
                .child(text)
        };
        let path_list = |paths: Vec<String>| {
            div()
                .mt(px(8.))
                .font_family(mono())
                .text_size(px(fs_small()))
                .text_color(p.secondary)
                .children(paths.into_iter().map(|path| div().child(path)))
        };
        if let PresetConfirmation::Conflicts(paths) = &confirmation {
            return self.action_dialog(
                "preset-file-conflicts",
                t!("presets.conflicts_title").to_string(),
                div()
                    .child(message(t!("presets.conflicts_hint").to_string()))
                    .child(path_list(
                        paths
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect(),
                    )),
                t!("presets.overwrite").to_string(),
                true,
                true,
                t!("presets.skip").to_string(),
                |this, cx| {
                    this.resolve_file_conflicts(crate::preset::FileConflictDecision::Skip, cx)
                },
                |this, cx| this.confirm_preset_action(cx),
                cx,
            );
        }
        let (key, title, body, confirm) = match &confirmation {
            PresetConfirmation::Delete(id) => {
                let installed = self.presets_view.store.installations(id).next().is_some();
                (
                    "delete-preset",
                    t!("presets.delete_title").to_string(),
                    div()
                        .child(message(
                            t!("install.delete_confirm", name = name(id)).to_string(),
                        ))
                        .when(installed, |body| {
                            body.child(hint(t!("presets.delete_hint").to_string()))
                        }),
                    t!("common.delete").to_string(),
                )
            }
            PresetConfirmation::Uninstall(id, _) => (
                "uninstall-preset",
                t!("projects.remove_from_project").to_string(),
                div()
                    .child(message(
                        t!("install.project_remove_confirm", name = name(id)).to_string(),
                    ))
                    .child(hint(t!("presets.uninstall_hint").to_string())),
                t!("common.remove").to_string(),
            ),
            PresetConfirmation::Overwrite(_, projects) => {
                let home = dirs::home_dir();
                let names = projects
                    .iter()
                    .map(|project| {
                        if home.as_ref() == Some(project) {
                            t!("common.global").to_string()
                        } else {
                            display_path(project)
                        }
                    })
                    .collect::<Vec<_>>();
                (
                    "overwrite-preset",
                    t!("presets.overwrite_title").to_string(),
                    div()
                        .child(message(t!("presets.overwrite_hint").to_string()))
                        .child(
                            div()
                                .mt(px(8.))
                                .font_family(mono())
                                .text_size(px(fs_small()))
                                .text_color(p.secondary)
                                .children(names.into_iter().map(|name| div().child(name))),
                        ),
                    t!("presets.overwrite").to_string(),
                )
            }
            PresetConfirmation::DeleteFile(_, path) => (
                "delete-preset-file",
                t!("presets.delete_file_title").to_string(),
                div().child(message(
                    t!("install.delete_confirm", name = path.display()).to_string(),
                )),
                t!("common.delete").to_string(),
            ),
            PresetConfirmation::Conflicts(_) => unreachable!(),
        };
        self.action_dialog(
            key,
            title,
            body,
            confirm,
            true,
            true,
            t!("common.cancel").to_string(),
            |this, cx| this.close_dialog(cx),
            |this, cx| this.confirm_preset_action(cx),
            cx,
        )
    }
}
