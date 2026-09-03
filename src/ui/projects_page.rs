use super::*;

impl KitterApp {
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
            location_labels.push(self.tr("内置", "Built-in").to_string());
        }
        let locations = location_labels.join(" · ");
        let name = effective.name.clone();
        let description = effective.description.clone();
        let installed_for_delete = ProjectSkill {
            name: effective.name.clone(),
            installations: effective.direct_installations.clone(),
        };
        let can_delete = !installed_for_delete.installations.is_empty();
        let row_selector = format!("project-skill-{name}");
        let mut row = div()
            .id(ElementId::Name(row_selector.clone().into()))
            .debug_selector(move || row_selector)
            .h(px(60.))
            .px(px(4.))
            .border_b_1()
            .border_color(p.border)
            .flex()
            .items_center()
            .hover(move |row| row.bg(p.hover))
            .child(div().size(px(15.)).flex_none().child(Self::icon(
                "icons/package.svg",
                15.,
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
                                    .font_family(MONO)
                                    .text_size(px(13.))
                                    .child(effective.name.clone())
                                    .when(!description.is_empty(), |name| {
                                        let tooltip_description = description.clone();
                                        name.tooltip(move |window, cx| {
                                            let description = tooltip_description.clone();
                                            Tooltip::element(move |_, _| {
                                                div()
                                                    .w(px(320.))
                                                    .max_w(px(320.))
                                                    .font_family(MONO)
                                                    .whitespace_normal()
                                                    .line_height(relative(1.45))
                                                    .text_size(px(12.))
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
                            .truncate()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(locations),
                    ),
            )
            .child(
                self.effective_agent_badges(format!("project-{name}"), &effective.agents, cx)
                    .ml(px(12.)),
            );
        if can_delete {
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
                        this.delete_flow.confirmation = Some(DeleteConfirmation::ProjectSkill {
                            project: project_for_delete.clone(),
                            skill: installed_for_delete.clone(),
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
        let global_skills = global_root
            .as_ref()
            .and_then(|path| self.project_snapshot(path, cx));
        let global_skill_count = global_skills.as_ref().map(Vec::len);
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
                let matches_tag = self.tags_flow.selected_project_filter.is_none_or(|tag| {
                    self.tags_flow
                        .projects
                        .matches_filter(&project_tag_key(path), tag)
                });
                matches_query && matches_tag
            })
            .collect::<Vec<_>>();
        let visible_project_count = projects.len();
        let project_context_app = cx.entity().downgrade();
        let mut recent_rows = div()
            .id("projects-scroll")
            .flex_1()
            .min_h_0()
            .px(px(8.))
            .overflow_y_scroll();
        if projects.is_empty() {
            recent_rows = recent_rows.child(
                div()
                    .mt(px(30.))
                    .px(px(16.))
                    .text_size(px(13.))
                    .text_color(p.muted)
                    .text_center()
                    .child(if self.model.library.config.recent_projects.is_empty() {
                        self.tr("还没有项目", "No projects yet")
                    } else {
                        self.tr("没有匹配的项目", "No matching projects")
                    }),
            );
        }
        for path in projects {
            let selected_path = path.clone();
            let selected = !self.projects_view.global_project_view
                && self.projects_view.open_project.as_ref() == Some(&path);
            let project_key = project_tag_key(&path);
            let assigned_project_tags = self
                .tags_flow
                .projects
                .assigned_tags(&project_key)
                .into_iter()
                .filter_map(|tag| self.tags_flow.projects.path(tag.id))
                .collect::<Vec<_>>();
            let mut project_tag_chips = div().mt(px(4.)).flex().gap(px(4.)).flex_wrap();
            for (index, tag) in assigned_project_tags.iter().enumerate() {
                project_tag_chips = project_tag_chips.child(
                    div()
                        .id(ElementId::Name(
                            format!("project-tag-{}-{index}", path.display()).into(),
                        ))
                        .px(px(3.))
                        .rounded(px(5.))
                        .font_family(MONO)
                        .text_size(px(10.))
                        .text_color(p.secondary)
                        .child(format!("#{tag}")),
                );
            }
            let reveal_path = path.clone();
            let set_tags_app = project_context_app.clone();
            let set_tags_path = path.clone();
            let remove_app = project_context_app.clone();
            let remove_path = path.clone();
            let set_tags_label = self.tr("设置标签", "Set Tags").to_string();
            let reveal_label = if cfg!(target_os = "macos") {
                self.tr("在访达中显示", "Show in Finder")
            } else {
                self.tr("在文件夹中显示", "Show in Folder")
            }
            .to_string();
            let remove_label = self.tr("移除项目", "Remove Project").to_string();
            let remove_menu_color = p.danger;
            recent_rows = recent_rows.child(
                div()
                    .id(ElementId::Name(
                        format!("project-{}", path.display()).into(),
                    ))
                    .min_h(px(48.))
                    .px(px(8.))
                    .my(px(2.))
                    .rounded(px(RADIUS_CONTROL))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(if selected {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |row| row.bg(p.hover))
                    .child(
                        div()
                            .size(px(28.))
                            .flex_none()
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Self::icon("icons/folder.svg", 16., p.secondary)),
                    )
                    .child(
                        div()
                            .ml(px(10.))
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
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
                                    .mt(px(3.))
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(display_path(&path)),
                            )
                            .when(!assigned_project_tags.is_empty(), |content| {
                                content.child(project_tag_chips)
                            }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.projects_view.open_project = Some(selected_path.clone());
                        this.projects_view.global_project_view = false;
                        this.projects_view.selected_project_agent = None;
                        this.projects_view.project_agents_expanded = false;
                        cx.notify();
                    }))
                    .context_menu(move |menu, _, _| {
                        let reveal_path = reveal_path.clone();
                        let set_tags_app = set_tags_app.clone();
                        let set_tags_path = set_tags_path.clone();
                        let remove_app = remove_app.clone();
                        let remove_path = remove_path.clone();
                        let remove_menu_item =
                            Self::danger_menu_item(remove_label.clone(), remove_menu_color);
                        menu.min_w(px(160.))
                            .item(
                                PopupMenuItem::new(set_tags_label.clone())
                                    .icon(Icon::default().path("icons/hash.svg"))
                                    .on_click(move |_, _, cx| {
                                        let _ = set_tags_app.update(cx, |this, cx| {
                                            this.open_project_tag_assignment_dialog(
                                                set_tags_path.clone(),
                                                cx,
                                            );
                                        });
                                    }),
                            )
                            .item(
                                PopupMenuItem::new(reveal_label.clone())
                                    .icon(Icon::new(IconName::Folder))
                                    .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
                            )
                            .item(remove_menu_item.on_click(move |_, window, cx| {
                                let _ = remove_app.update(cx, |this, cx| {
                                    this.model.library.config.remove_project(&remove_path);
                                    if this.projects_view.open_project.as_ref()
                                        == Some(&remove_path)
                                    {
                                        this.projects_view.open_project = None;
                                        this.projects_view.global_project_view = true;
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
        let global_row = div()
            .id("global-project-skills")
            .min_h(px(52.))
            .mx(px(8.))
            .mb(px(6.))
            .px(px(8.))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(if global_selected {
                p.selected
            } else {
                rgba(0x00000000)
            })
            .hover(move |row| row.bg(p.hover))
            .child(
                div()
                    .size(px(28.))
                    .flex_none()
                    .rounded(px(RADIUS_CONTROL))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Self::icon("icons/house.svg", 16., p.secondary)),
            )
            .child(
                div()
                    .ml(px(10.))
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.tr("全局生效", "Global")),
                    )
                    .child(
                        div()
                            .mt(px(3.))
                            .font_family(MONO)
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .truncate()
                            .child(match global_skill_count {
                                Some(count) if self.uses_english() => {
                                    format!("{count} user-level Skills")
                                }
                                Some(count) => format!("{count} 个用户级技能"),
                                None => self.tr("正在后台扫描…", "Scanning in background…").into(),
                            }),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.projects_view.global_project_view = true;
                this.projects_view.selected_project_agent = None;
                this.projects_view.project_agents_expanded = false;
                cx.notify();
            }));
        let recent_panel = div()
            .w_full()
            .min_w_0()
            .min_h_0()
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(p.surface)
            .child(
                self.panel_header(
                    "projects-panel-window-drag",
                    self.tr("项目", "Projects"),
                    self.model.library.config.project_paths().len(),
                    cx,
                )
                .child(
                    self.labeled_icon_button(
                        "open-project-folder",
                        "icons/plus.svg",
                        p.text,
                        self.tr("打开项目文件夹", "Open project folder"),
                        cx,
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.browse_project(window, cx)),
                    ),
                ),
            )
            .child(
                div().px(px(10.)).py(px(8.)).child(
                    Input::new(&self.projects_view.project_search)
                        .small()
                        .h(px(SEARCH_HEIGHT))
                        .w_full()
                        .rounded(px(999.))
                        .bg(p.surface)
                        .border_color(p.border_strong)
                        .text_size(px(14.))
                        .prefix(Self::icon("icons/search.svg", 16., p.muted)),
                ),
            )
            .child(global_row)
            .child(
                div()
                    .px(px(16.))
                    .pb(px(6.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(
                        self.tags_flow
                            .selected_project_filter
                            .and_then(|tag| self.tags_flow.projects.path(tag))
                            .map(|path| format!("#{path}  {visible_project_count}"))
                            .unwrap_or_else(|| {
                                format!("{}  {visible_project_count}", self.tr("项目", "Projects"))
                            }),
                    )
                    .child(div().flex_1())
                    .child(self.tag_filter_control(TagScope::Projects, cx)),
            )
            .child(recent_rows);

        let selected_root = if self.projects_view.global_project_view {
            global_root.as_ref()
        } else {
            self.projects_view.open_project.as_ref()
        };
        let detail = if let Some(open_project) = selected_root {
            let is_global = self.projects_view.global_project_view;
            let project_skills = if is_global {
                global_skills
            } else {
                self.project_snapshot(open_project, cx)
            };
            let estimates = project_skills
                .as_ref()
                .and_then(|_| self.context_estimate_snapshot(open_project, cx));
            let loading = project_skills.is_none() || estimates.is_none();
            let project_skills = project_skills.unwrap_or_default();
            let estimates = estimates.unwrap_or_default();
            let context_panel = if loading {
                div()
                    .h(px(86.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(9.))
                    .text_size(px(13.))
                    .text_color(p.muted)
                    .child(self.shell.spinner_accent.clone())
                    .child(self.tr("正在后台扫描项目…", "Scanning project in background…"))
            } else {
                self.context_estimate_panel(open_project, &estimates, cx)
            };
            let effective_rows = effective_skill_rows(
                &estimates,
                &project_skills,
                self.projects_view.selected_project_agent,
            );
            let skill_count = effective_rows.len();
            let effective_rows = Arc::new(effective_rows);
            let mut skills = div()
                .id("project-skills-scroll")
                .debug_selector(|| "project-skills-scroll".into())
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .px(px(28.))
                .pt(px(14.))
                .pb(px(28.));
            if effective_rows.is_empty() && !loading {
                let selected_agent = self
                    .projects_view
                    .selected_project_agent
                    .map(AgentKind::label)
                    .unwrap_or_default();
                skills = skills.child(
                    div()
                        .mt(px(70.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_color(p.muted)
                        .child(Self::icon("icons/package.svg", 28., p.muted))
                        .child(div().mt(px(10.)).text_size(px(14.)).child(
                            if self.projects_view.selected_project_agent.is_some() {
                                if self.uses_english() {
                                    format!("No effective Skills for {selected_agent}")
                                } else {
                                    format!("没有检测到对 {selected_agent} 生效的技能")
                                }
                            } else if is_global {
                                self.tr(
                                    "没有检测到用户级生效的技能",
                                    "No user-level Skills detected",
                                )
                                .to_string()
                            } else {
                                self.tr(
                                    "没有检测到对这个项目生效的技能",
                                    "No effective Skills detected for this project",
                                )
                                .to_string()
                            },
                        )),
                );
            }
            if !effective_rows.is_empty() {
                let rows = Arc::clone(&effective_rows);
                let project = open_project.clone();
                skills = skills.child(
                    uniform_list(
                        "project-skills-virtual-list",
                        rows.len(),
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| this.project_skill_row(&rows[index], &project, cx))
                                .collect()
                        }),
                    )
                    .flex_1()
                    .min_h_0(),
                );
            }
            let plugin_groups =
                effective_plugin_groups(&estimates, self.projects_view.selected_project_agent);
            let plugin_skill_count = plugin_groups
                .iter()
                .map(|plugin| plugin.skills.len())
                .sum::<usize>();
            let tabs = self.project_skills_tabs(skill_count, plugin_skill_count, cx);
            let skills = match self.projects_view.project_skills_tab {
                ProjectSkillsTab::Skills => skills,
                ProjectSkillsTab::Plugins => self.effective_plugins_list(plugin_groups, cx),
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
                        .pb(px(18.))
                        .border_b_1()
                        .border_color(p.border)
                        .child(
                            div()
                                .font_family(MONO)
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.selectable_text(
                                    "project-detail-name",
                                    300,
                                    if is_global {
                                        self.tr("全局生效", "Global").to_string()
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
                        )
                        .when(!is_global, |header| {
                            header.child(
                                div()
                                    .mt(px(4.))
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(self.selectable_text(
                                        "project-detail-path",
                                        301,
                                        display_path(open_project),
                                        window,
                                        cx,
                                    )),
                            )
                        }),
                )
                .child(
                    div()
                        .px(px(28.))
                        .pt(px(10.))
                        .pb(px(12.))
                        .flex_none()
                        .border_b_1()
                        .border_color(p.border)
                        .child(context_panel),
                )
                .child(tabs)
                .child(skills)
                .child(self.window_drag_strip("project-detail-window-drag", 24., cx))
        } else {
            let first = self.model.library.config.recent_projects.is_empty();
            div()
                .relative()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(48.))
                        .rounded(px(RADIUS_CARD))
                        .bg(p.raised)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Self::icon("icons/folder.svg", 22., p.secondary)),
                )
                .child(
                    div()
                        .mt(px(16.))
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(if first {
                            self.tr("打开第一个项目文件夹", "Open your first project folder")
                        } else {
                            self.tr("打开一个项目文件夹", "Open a project folder")
                        }),
                )
                .child(
                    div()
                        .mt(px(7.))
                        .max_w(px(430.))
                        .text_center()
                        .text_size(px(14.))
                        .line_height(relative(1.55))
                        .text_color(p.muted)
                        .child(self.tr(
                            "Kitter 会读取项目里的技能，并让你管理它们的安装边界。",
                            "Kitter reads the Skills in a project and lets you manage where they are installed.",
                        )),
                )
                .child(
                    self.primary_icon_button(
                        "empty-open-project",
                        "icons/folder.svg",
                        self.tr("打开项目文件夹", "Open project folder"),
                        cx,
                    )
                    .mt(px(17.))
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
