use super::*;

impl KitterApp {
    pub(super) fn install_modal(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let has_target = !self.install_flow.selected_targets.is_empty()
            && if self.install_flow.global {
                dirs::home_dir().is_some()
            } else {
                self.projects_view.open_project.is_some()
            };
        let selected_keys = self.selected_skill_keys();
        let selected_count = selected_keys.len();
        let selected_label = if selected_count > 1 {
            if self.uses_english() {
                format!("{} Skills", selected_count)
            } else {
                format!("{} 个技能", selected_count)
            }
        } else {
            self.selected_skill()
                .map(|skill| skill.record.name.clone())
                .unwrap_or_else(|| self.tr("技能", "Skill").to_string())
        };
        let global_label = self.tr("全局", "Global").to_string();
        let project_label = if self.install_flow.global {
            global_label.clone()
        } else {
            self.projects_view
                .open_project
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.tr("选择一个项目", "Choose a project").to_string())
        };
        let selected_global = self.install_flow.global;
        let selected_project = self.projects_view.open_project.clone();
        let home = dirs::home_dir();
        let project_choices = home
            .iter()
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
            .trigger(
                self.dropdown_button("install-project-select", project_label, 14.)
                    .w_full(),
            )
            .content(move |_, _, popover_cx| {
                let mut menu_background = p.elevated;
                menu_background.a = 1.;
                let mut menu = div()
                    .w(px(518.))
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(menu_background)
                    .shadow_lg();
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
                        div()
                            .id(ElementId::Name(
                                format!("install-project-option-{index}").into(),
                            ))
                            .min_h(px(46.))
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .cursor_pointer()
                            .bg(if active { p.selected } else { rgba(0x00000000) })
                            .hover(move |item| item.bg(p.hover))
                            .child(
                                div()
                                    .font_family(FONT_UI)
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(path_label),
                            )
                            .on_click(popover_cx.listener(move |_, _, _, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.install_flow.global = global;
                                    if !global {
                                        this.projects_view.open_project = Some(path.clone());
                                    }
                                    this.notify_dialog(cx);
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                menu
            });
        let project_picker = div().flex_1().child(project_control);
        let close_button = div()
            .id("close-install-modal")
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(p.raised)
            .hover(move |button| button.bg(p.hover))
            .text_color(p.text)
            .child(Self::icon("icons/x.svg", 14., p.text))
            .on_click(cx.listener(|this, _, _, cx| {
                this.install_flow.modal = false;
                this.close_dialog(cx);
            }));
        let install_content = div()
            .id("install-modal-content")
            .px(px(20.))
            .pb(px(20.))
            .max_h(px(500.))
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(p.elevated)
            .child(
                div()
                    .mb(px(7.))
                    .text_size(px(14.))
                    .text_color(p.muted)
                    .child(self.tr("安装到", "Install to")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(project_picker)
                    .child(
                        self.bordered_icon_button("modal-browse", "icons/folder.svg", p.text, cx)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.browse_project(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(18.))
                    .mb(px(7.))
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(self.tr("安装位置", "Install location")),
            )
            .child(self.target_group(cx));
        let panel = div()
            .relative()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .rounded_tl(px(RADIUS_MODAL))
                    .rounded_tr(px(RADIUS_MODAL))
                    .bg(p.elevated)
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(14.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .font_family(FONT_UI)
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if self.uses_english() {
                                        format!("Install {}", selected_label.clone())
                                    } else {
                                        format!("安装 {}", selected_label.clone())
                                    }),
                            )
                            .child(div().flex_1())
                            .child(close_button),
                    ),
            )
            .child(install_content)
            .child(
                div()
                    .h(px(60.))
                    .rounded_bl(px(RADIUS_MODAL))
                    .rounded_br(px(RADIUS_MODAL))
                    .px(px(20.))
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.elevated)
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("cancel-install")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(14.))
                            .hover(move |button| button.bg(p.hover))
                            .child(self.tr("取消", "Cancel"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.install_flow.modal = false;
                                    this.close_dialog(cx);
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("confirm-install")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .border_1()
                            .border_color(p.border)
                            .bg(if has_target { p.accent_fill } else { p.raised })
                            .text_color(if has_target { p.on_accent } else { p.muted })
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(14.))
                            .child(self.tr("安装", "Install"))
                            .when(has_target, |el| {
                                el.hover(move |button| button.opacity(0.82)).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| this.install_selected(cx)),
                                )
                            }),
                    ),
            );
        let _ = window;
        panel
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
            .contains(&InstallTarget::Universal);
        let agent_cell = |name: &'static str, icon_path: &'static str, right_border: bool| {
            div()
                .h(px(48.))
                // Keep the three-column grid stable when the final row has
                // fewer than three Agents.
                .flex_basis(relative(1. / 3.))
                .flex_shrink_0()
                .px(px(11.))
                .border_t_1()
                .border_color(p.border)
                .when(right_border, |cell| {
                    cell.border_r_1().border_color(p.border)
                })
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .size(px(28.))
                        .rounded(px(RADIUS_CONTROL))
                        .bg(if icon_path == "icons/provider-codex.svg" {
                            rgba(0x00000000)
                        } else {
                            p.raised
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Self::brand_icon(icon_path, 18., p.text)),
                )
                .child(div().text_size(px(12.)).text_color(p.secondary).child(name))
        };
        let universal_agents = crate::agents::AGENT_ICON_ORDER
            .iter()
            .filter(|agent| !agent.global_only && agent.supports_target(InstallTarget::Universal))
            .collect::<Vec<_>>();
        let mut universal_rows = div().flex().flex_col();
        for chunk in universal_agents.chunks(3) {
            let mut row = div().flex();
            for (index, agent) in chunk.iter().enumerate() {
                row = row.child(agent_cell(agent.name, agent.icon_path, index % 3 < 2));
            }
            for index in chunk.len()..3 {
                row = row.child(
                    div()
                        .h(px(48.))
                        .flex_basis(relative(1. / 3.))
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(p.border)
                        .when(index < 2, |cell| cell.border_r_1().border_color(p.border)),
                );
            }
            universal_rows = universal_rows.child(row);
        }
        let universal_box = div()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                div()
                    .id("target-universal")
                    .h(px(48.))
                    .px(px(13.))
                    .cursor_pointer()
                    .bg(if universal { p.selected } else { p.surface })
                    .when(universal, |row| {
                        row.rounded_tl(px(RADIUS_CARD - 1.))
                            .rounded_tr(px(RADIUS_CARD - 1.))
                    })
                    .flex()
                    .items_center()
                    .child(Checkbox::new("target-universal-checkbox").checked(universal))
                    .child(
                        div()
                            .ml(px(9.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.tr("通用", "Universal")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(if self.install_flow.global {
                                "~/.agents/skills"
                            } else {
                                ".agents/skills"
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_install_target(InstallTarget::Universal);
                        this.notify_dialog(cx);
                    })),
            )
            .child(universal_rows);
        let mut independent_rows = div().flex().flex_col();
        for info in crate::agents::INDEPENDENT_INSTALL_TARGETS {
            independent_rows = independent_rows.child(self.target_row(*info, cx));
        }
        let independent_box = div()
            .mt(px(10.))
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                div()
                    .px(px(13.))
                    .h(px(42.))
                    .flex()
                    .items_center()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.tr("独立安装", "Agent-specific")),
            )
            .child(independent_rows);
        div().child(universal_box).child(independent_box)
    }

    pub(super) fn target_row(
        &self,
        info: crate::agents::InstallTargetInfo,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let id = match info.target {
            InstallTarget::Codex => "target-codex",
            InstallTarget::ClaudeCode => "target-claude",
            InstallTarget::Cursor => "target-cursor",
            InstallTarget::OpenCode => "target-opencode",
            InstallTarget::Pi => "target-pi",
            InstallTarget::Grok => "target-grok",
            InstallTarget::Antigravity => "target-antigravity",
            InstallTarget::Droid => "target-droid",
            InstallTarget::Copilot => "target-copilot",
            InstallTarget::Universal => "target-universal",
        };
        let active = self.install_flow.selected_targets.contains(&info.target);
        let is_last = crate::agents::INDEPENDENT_INSTALL_TARGETS
            .last()
            .is_some_and(|last| last.target == info.target);
        div()
            .id(id)
            .h(px(48.))
            .px(px(13.))
            .border_t_1()
            .border_color(p.border)
            .bg(if active { p.selected } else { p.surface })
            .when(is_last, |row| {
                row.rounded_bl(px(RADIUS_CARD - 1.))
                    .rounded_br(px(RADIUS_CARD - 1.))
            })
            .flex()
            .items_center()
            .cursor_pointer()
            .child(Checkbox::new(ElementId::Name(format!("{id}-checkbox").into())).checked(active))
            .child(
                div()
                    .ml(px(9.))
                    .size(px(28.))
                    .rounded(px(RADIUS_CONTROL))
                    .bg(if info.icon_path == "icons/provider-codex.svg" {
                        rgba(0x00000000)
                    } else {
                        p.raised
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Self::brand_icon(info.icon_path, 18., p.text)),
            )
            .child(div().ml(px(9.)).text_size(px(13.)).child(info.name))
            .child(div().flex_1())
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(if self.install_flow.global {
                        dirs::home_dir()
                            .map(|home| {
                                display_path(&crate::agents::global_target_root(&home, info.target))
                            })
                            .unwrap_or_default()
                    } else {
                        crate::agents::target_directory(info.target).to_string()
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_install_target(info.target);
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
                        if self.uses_english() {
                            format!("Delete “{label}”?")
                        } else {
                            format!("删除「{label}」？")
                        }
                    } else if self.uses_english() {
                        format!("Delete {} Skills?", labels.len())
                    } else {
                        format!("删除 {} 个技能？", labels.len())
                    },
                    (if linked_count == skills.len() {
                        self.tr("仅移除 Kitter 托管与安装链接，原始源目录保留。", "Remove Kitter entries and installation links. Original source folders are kept.")
                    } else if linked_count > 0 {
                        self.tr("托管副本与安装将被删除；链接来源的原始目录保留。", "Managed copies and installations will be removed. Linked source folders are kept.")
                    } else { self.tr(
                        "这会从 Kitter 中删除这些技能，同时移除它们在项目中的安装。以下位置的文件也会被删除，且无法恢复：",
                        "This removes these Skills from Kitter and from projects where they are installed. Files at these locations will also be deleted and cannot be restored:",
                    ) })
                    .to_string(),
                    Some(
                        skills
                            .iter()
                            .map(|(_, path)| display_path(path))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    self.tr("删除", "Delete"),
                    self.tr("技能已删除", "Skills deleted"),
                )
            }
            DeleteConfirmation::ProjectSkill { project, skill } => {
                let global_scope = dirs::home_dir()
                    .as_ref()
                    .is_some_and(|home| home == project);
                let kinds = skill
                    .installations
                    .iter()
                    .filter(|installation| self.delete_flow.selected.contains(&installation.path))
                    .map(project::removal_kind)
                    .collect::<Vec<_>>();
                let deletes_files = kinds
                    .iter()
                    .any(|kind| matches!(kind, project::RemovalKind::SourceFiles));
                let external_sources = unique_external_sources(&kinds);
                let message = if deletes_files {
                    if global_scope {
                        self.tr(
                            "所选用户级安装中包含直接保存的技能，确认后这些文件会被删除，且无法恢复。",
                            "Some selected user-level installations are stored directly there. Their files will be deleted and cannot be restored.",
                        )
                    } else {
                        self.tr(
                            "所选位置中包含直接保存在项目里的技能，确认后这些文件会被删除，且无法恢复。",
                            "Some selected installations are stored directly in the project. Their files will be deleted and cannot be restored.",
                        )
                    }
                } else if !external_sources.is_empty() {
                    if global_scope {
                        self.tr(
                            "只会移除选中的用户级安装，原始技能文件不会受到影响。",
                            "Only the selected user-level installations will be removed. The original Skill files will not be affected.",
                        )
                    } else {
                        self.tr(
                            "只会移除这个项目中的安装，原始技能文件不会受到影响。",
                            "Only the project installations will be removed. The original Skill files will not be affected.",
                        )
                    }
                } else {
                    if global_scope {
                        self.tr(
                            "只会移除选中的用户级安装。Kitter 中保存的技能不会被删除，你之后仍可以再次安装。",
                            "Only the selected user-level installations will be removed. The Skill saved in Kitter will remain available to install again.",
                        )
                    } else {
                        self.tr(
                            "只会移除这个项目中的安装。Kitter 中保存的技能不会被删除，你之后仍可以再次安装。",
                            "Only the project installations will be removed. The Skill saved in Kitter will remain available to install again.",
                        )
                    }
                };
                (
                    if global_scope && self.uses_english() {
                        format!("Remove “{}” from user-level locations?", skill.name)
                    } else if global_scope {
                        format!("从用户级目录中移除「{}」？", skill.name)
                    } else if self.uses_english() {
                        format!("Remove “{}” from this project?", skill.name)
                    } else {
                        format!("从这个项目中移除「{}」？", skill.name)
                    },
                    message.to_string(),
                    (!external_sources.is_empty()).then(|| external_sources.join("\n")),
                    if deletes_files {
                        self.tr("删除", "Delete")
                    } else {
                        self.tr("移除", "Remove")
                    },
                    if global_scope {
                        self.tr("已从用户级目录中移除", "Removed from user-level locations")
                    } else {
                        self.tr("已从项目中移除", "Removed from project")
                    },
                )
            }
        };

        let close_button = div()
            .id("close-delete-modal")
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(p.raised)
            .hover(move |button| button.bg(p.hover))
            .text_color(p.text)
            .child(Self::icon("icons/x.svg", 14., p.text))
            .on_click(cx.listener(|this, _, _, cx| {
                this.delete_flow.confirmation = None;
                this.close_dialog(cx);
            }));
        let mut body = div()
            .rounded_tl(px(RADIUS_MODAL))
            .rounded_tr(px(RADIUS_MODAL))
            .bg(p.elevated)
            .p(px(20.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().flex_1())
                    .child(close_button),
            )
            .child(
                div()
                    .mt(px(10.))
                    .text_size(px(14.))
                    .line_height(relative(1.55))
                    .text_color(p.secondary)
                    .child(message),
            )
            .when_some(location, |body, location| {
                body.child(
                    div()
                        .mt(px(10.))
                        .px(px(11.))
                        .py(px(9.))
                        .rounded(px(RADIUS_CONTROL))
                        .bg(p.raised)
                        .font_family(MONO)
                        .text_size(px(12.))
                        .text_color(p.secondary)
                        .child(location),
                )
            });
        if let DeleteConfirmation::ProjectSkill { skill, .. } = &confirmation {
            body = body.child(
                div()
                    .mt(px(16.))
                    .mb(px(6.))
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(self.tr("选择要移除的位置", "Choose installations to remove")),
            );
            let unique_paths = unique_installation_paths(&skill.installations);
            for installation in skill
                .installations
                .iter()
                .filter(|installation| unique_paths.contains(&installation.path))
            {
                let path = installation.path.clone();
                let checked = self.delete_flow.selected.contains(&path);
                let target = crate::agents::target_directory(installation.target);
                body = body.child(
                    div()
                        .id(ElementId::Name(
                            format!("delete-path-{}", path.display()).into(),
                        ))
                        .mt(px(6.))
                        .min_h(px(48.))
                        .px(px(11.))
                        .rounded(px(RADIUS_CARD))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.surface)
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
                                .child(div().text_size(px(13.)).child(target))
                                .child(
                                    div()
                                        .mt(px(2.))
                                        .font_family(MONO)
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(display_path(&path)),
                                ),
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
        let can_confirm = !self.delete_flow.busy
            && (!matches!(confirmation, DeleteConfirmation::ProjectSkill { .. })
                || !self.delete_flow.selected.is_empty());
        div()
            .relative()
            .rounded(px(RADIUS_MODAL))
            .bg(p.elevated)
            .overflow_hidden()
            .child(body)
            .child(
                div()
                    .h(px(60.))
                    .rounded_bl(px(RADIUS_MODAL))
                    .rounded_br(px(RADIUS_MODAL))
                    .px(px(18.))
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.elevated)
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("cancel-delete")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(14.))
                            .hover(move |button| button.bg(p.hover))
                            .child(self.tr("取消", "Cancel"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.delete_flow.confirmation = None;
                                    this.close_dialog(cx);
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("confirm-delete")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .border_1()
                            .border_color(p.border)
                            .bg(if can_confirm { p.danger } else { p.danger_soft })
                            .flex()
                            .items_center()
                            .cursor(if can_confirm {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::Arrow
                            })
                            .text_size(px(14.))
                            .text_color(if can_confirm { p.on_accent } else { p.muted })
                            .child(action)
                            .when(can_confirm, |button| {
                                button.on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
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
                                                    Err(error) => failures.push(error.to_string()),
                                                }
                                            }
                                            this.delete_flow.confirmation = None;
                                            this.close_dialog(cx);
                                            this.refresh(cx);
                                            if failures.is_empty() {
                                                this.show_notice(success, cx);
                                            } else if this.uses_english() {
                                                this.show_notice(
                                                    format!(
                                                        "Deleted {removed}; {} failed",
                                                        failures.len()
                                                    ),
                                                    cx,
                                                );
                                            } else {
                                                this.show_notice(
                                                    format!(
                                                        "已删除 {} 个，{} 个失败",
                                                        removed,
                                                        failures.len()
                                                    ),
                                                    cx,
                                                );
                                            }
                                        }
                                        DeleteConfirmation::ProjectSkill { project, skill } => {
                                            let report = project::remove_project_skills(
                                                skill.installations.iter().filter(|installation| {
                                                    this.delete_flow.selected
                                                        .contains(&installation.path)
                                                }),
                                            );
                                            let message = if report.failures.is_empty() {
                                                success.to_string()
                                            } else if report.removed > 0 {
                                                if this.uses_english() {
                                                    format!(
                                                        "Removed {} location(s); {} could not be removed",
                                                        report.removed,
                                                        report.failures.len()
                                                    )
                                                } else {
                                                    format!(
                                                        "已移除 {} 个位置，另有 {} 个位置未能移除",
                                                        report.removed,
                                                        report.failures.len()
                                                    )
                                                }
                                            } else if this.uses_english() {
                                                format!(
                                                    "{} location(s) could not be removed. Check access and try again",
                                                    report.failures.len()
                                                )
                                            } else {
                                                format!(
                                                    "{} 个位置未能移除，请检查访问权限后重试",
                                                    report.failures.len()
                                                )
                                            };
                                            if report.removed > 0 {
                                                this.projects_view.context_estimates
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
                                            this.close_dialog(cx);
                                            this.show_notice(message, cx);
                                            this.refresh(cx);
                                        }
                                    }
                                }))
                            }),
                    ),
            )
    }
}
