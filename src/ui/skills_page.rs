use super::*;

fn installable_skills_by_group(
    skills: &[SkillSummary],
    group_ids: &HashSet<&str>,
) -> BTreeMap<String, Vec<String>> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for skill in skills {
        if let Some(group_id) = skill.record.group_id.as_ref()
            && group_ids.contains(group_id.as_str())
        {
            grouped
                .entry(group_id.clone())
                .or_default()
                .push(skill_storage_name(skill).to_string());
        }
    }
    grouped
}

impl KitterApp {
    pub(super) fn skill_list_row(
        &self,
        skill: &SkillSummary,
        nested: bool,
        visible_order: Arc<[String]>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let name = skill.record.name.clone();
        let built_in = skill.record.origin.is_builtin();
        let storage_name = skill_storage_name(skill).to_string();
        let context_app = cx.entity().downgrade();
        let selection_count = self.selected_skill_keys().len();
        let selected = self.skills_view.selection.contains(&storage_name);
        let multi_selection = selected && selection_count > 1;
        let protected_selection = built_in
            || (multi_selection
                && self.model.skills.iter().any(|skill| {
                    skill.record.origin.is_builtin()
                        && self
                            .skills_view
                            .selection
                            .contains(skill_storage_name(skill))
                }));
        let install_label = if multi_selection {
            if self.uses_english() {
                format!("Install {selection_count} Skills")
            } else {
                format!("安装 {} 个技能", selection_count)
            }
        } else {
            self.tr("安装技能", "Install Skill").to_string()
        };
        let set_tags_label = if multi_selection {
            if self.uses_english() {
                format!("Set Tags ({selection_count})")
            } else {
                format!("设置标签（{}）", selection_count)
            }
        } else {
            self.tr("设置标签", "Set Tags").to_string()
        };
        let reveal_label = if cfg!(target_os = "macos") {
            self.tr("在访达中显示", "Show in Finder")
        } else {
            self.tr("在文件夹中显示", "Show in Folder")
        }
        .to_string();
        let delete_label = if multi_selection {
            if self.uses_english() {
                format!("Delete {selection_count} Skills")
            } else {
                format!("删除 {} 个技能", selection_count)
            }
        } else {
            self.tr("删除技能", "Delete Skill").to_string()
        };
        let move_group_label = if multi_selection {
            if self.uses_english() {
                format!("Move {selection_count} Skills to Group")
            } else {
                format!("移动 {} 个技能到分组", selection_count)
            }
        } else {
            self.tr("移动分组", "Move to Group").to_string()
        };
        let delete_menu_color = p.danger;
        let selection_mode = self.skills_view.selection.is_multiple();
        let context_name = storage_name.clone();
        let click_name = storage_name.clone();
        let click_order = visible_order;
        let reveal_path = skill.path.clone();
        let checkbox_name = storage_name.clone();
        let checkbox = div()
            .id(ElementId::Name(
                format!("select-skill-{storage_name}").into(),
            ))
            .size(px(18.))
            .rounded(px(5.))
            .border_1()
            .border_color(if selected { p.accent } else { p.border_strong })
            .bg(if selected { p.accent } else { rgba(0x00000000) })
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(selected, |cell| {
                cell.child(Self::icon("icons/check.svg", 13., p.on_accent))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_skill_selection(checkbox_name.clone(), cx);
            }));
        div()
            .id(ElementId::Name(format!("skill-{storage_name}").into()))
            .min_h(px(44.))
            .px(px(8.))
            .my(px(2.))
            .ml(px(if nested { 12. } else { 0. }))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .bg(if selected {
                p.selected
            } else {
                rgba(0x00000000)
            })
            .hover(move |row| row.bg(p.hover))
            .when(selection_mode, |row| row.child(checkbox))
            .child(Self::icon(
                if built_in {
                    "icons/crown.svg"
                } else {
                    "icons/package.svg"
                },
                16.,
                p.secondary,
            ))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(name.clone()),
                    )
                    .child(
                        div().text_size(px(12.)).text_color(p.muted).child(
                            if dirs::home_dir()
                                .is_some_and(|home| self.skill_installed_at(skill, &home, cx))
                            {
                                if skill.installed_projects == 0 {
                                    self.tr("已全局安装", "Installed globally").into()
                                } else if self.uses_english() {
                                    format!("Global · {} projects", skill.installed_projects)
                                } else {
                                    format!("全局 · {} 个项目", skill.installed_projects)
                                }
                            } else if skill.installed_projects == 0 {
                                self.tr("尚未安装", "Not installed").into()
                            } else if self.uses_english() {
                                format!("Installed in {} projects", skill.installed_projects)
                            } else {
                                format!("已安装到 {} 个项目", skill.installed_projects)
                            },
                        ),
                    ),
            )
            .when(skill.manual_only, |row| {
                row.child(self.manual_skill_badge())
            })
            .when(!built_in, |row| {
                row.on_drag(
                    SkillDrag {
                        name: storage_name.clone(),
                    },
                    |drag, _, _, cx| cx.new(|_| drag.clone()),
                )
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    this.prepare_skill_context_selection(context_name.clone(), cx);
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                this.select_skill_from_click(
                    click_name.clone(),
                    event.modifiers(),
                    click_order.as_ref(),
                    cx,
                );
            }))
            .context_menu(move |menu, _, _| {
                let install_app = context_app.clone();
                let set_tags_app = context_app.clone();
                let move_app = context_app.clone();
                let delete_app = context_app.clone();
                let mut menu = menu.min_w(px(210.)).item(
                    PopupMenuItem::new(install_label.clone())
                        .icon(Icon::default().path("icons/download.svg"))
                        .on_click(move |_, window, cx| {
                            let _ = install_app.update(cx, |this, cx| {
                                this.open_install_dialog(window, cx);
                            });
                        }),
                );
                if !multi_selection {
                    let reveal_path = reveal_path.clone();
                    menu = menu.item(
                        PopupMenuItem::new(reveal_label.clone())
                            .icon(Icon::new(IconName::FolderOpen))
                            .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
                    );
                }
                menu = menu.item(
                    PopupMenuItem::new(set_tags_label.clone())
                        .icon(Icon::default().path("icons/hash.svg"))
                        .on_click(move |_, _, cx| {
                            let _ = set_tags_app.update(cx, |this, cx| {
                                let keys = this.selected_skill_keys();
                                this.open_tag_assignment_dialog_for_selection(keys, cx);
                            });
                        }),
                );
                if !protected_selection {
                    menu = menu.item(
                        PopupMenuItem::new(move_group_label.clone())
                            .icon(Icon::new(IconName::Folder))
                            .on_click(move |_, _, cx| {
                                let _ = move_app.update(cx, |this, cx| {
                                    let keys = this.selected_skill_keys();
                                    this.open_move_group_dialog_for_selection(keys, cx);
                                });
                            }),
                    );
                    let delete_menu_item =
                        Self::danger_menu_item(delete_label.clone(), delete_menu_color).on_click(
                            move |_, window, cx| {
                                let _ = delete_app.update(cx, |this, cx| {
                                    let targets = this.selected_library_targets();
                                    if targets.is_empty() {
                                        return;
                                    }
                                    this.delete_flow.selected.clear();
                                    this.delete_flow.confirmation =
                                        Some(DeleteConfirmation::LibrarySkills { skills: targets });
                                    this.open_delete_dialog(window, cx);
                                });
                            },
                        );
                    menu = menu.item(delete_menu_item);
                }
                menu
            })
            .into_any_element()
    }

    pub(super) fn group_inline_editor(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let can_save = !self
            .groups_flow
            .name_input
            .read(cx)
            .value()
            .trim()
            .is_empty();
        div()
            .id(ElementId::Name(id.to_string().into()))
            .min_h(px(34.))
            .pl(px(6.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Input::new(&self.groups_flow.name_input)
                    .small()
                    .h(px(30.))
                    .min_w_0()
                    .flex_1()
                    .rounded(px(RADIUS_INLINE_INPUT))
                    .bg(p.surface)
                    .border_color(p.border_strong)
                    .text_size(px(13.)),
            )
            .child(
                div()
                    .id("cancel-group-edit")
                    .size(px(24.))
                    .rounded(px(7.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |button| button.bg(p.hover))
                    .child(Self::icon("icons/x.svg", 13., p.muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.groups_flow.edit = None;
                        this.tags_flow.error = None;
                        this.notify_dialog(cx);
                    })),
            )
            .child(
                div()
                    .id("save-group-edit")
                    .size(px(24.))
                    .rounded(px(7.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(if can_save {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::Arrow
                    })
                    .text_color(if can_save { p.text } else { p.muted })
                    .when(can_save, |button| {
                        button
                            .hover(move |button| button.bg(p.hover))
                            .on_click(cx.listener(|this, _, _, cx| this.commit_group_edit(cx)))
                    })
                    .child(Self::icon(
                        "icons/check.svg",
                        13.,
                        if can_save { p.text } else { p.muted },
                    )),
            )
            .into_any_element()
    }

    pub(super) fn group_management_row(
        &self,
        group: SkillGroup,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.groups_flow.edit == Some(GroupEdit::Rename(group.id.clone())) {
            return self.group_inline_editor(&format!("rename-group-{}", group.id), cx);
        }
        let p = self.palette();
        let group_id = group.id.clone();
        let rename_group_id = group_id.clone();
        let delete_group_id = group_id.clone();
        let group_name = group.name.clone();
        let count = self
            .model
            .skills
            .iter()
            .filter(|skill| skill.record.group_id.as_deref() == Some(group.id.as_str()))
            .count();
        let group_key = SharedString::from(format!("group-management-row-{}", group.id));
        let mut actions = div()
            .invisible()
            .group_hover(group_key.clone(), |actions| actions.visible())
            .flex()
            .items_center();
        actions = actions.child(
            div()
                .id(ElementId::Name(format!("delete-group-{}", group_id).into()))
                .size(px(24.))
                .rounded(px(7.5))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |button| button.bg(p.danger_soft))
                .child(Self::icon("icons/x.svg", 13., p.danger))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_group_delete_dialog(delete_group_id.clone(), cx);
                })),
        );
        div()
            .id(ElementId::Name(format!("group-row-{}", group_id).into()))
            .group(group_key)
            .h(px(34.))
            .pl(px(10.))
            .pr(px(6.))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .gap(px(7.))
            .hover(move |row| row.bg(p.hover))
            .child(Self::icon("icons/folder.svg", 15., p.secondary))
            .child(
                div()
                    .id(ElementId::Name(
                        format!("edit-group-name-{}", group_id).into(),
                    ))
                    .min_w_0()
                    .flex_1()
                    .font_family(MONO)
                    .text_size(px(13.))
                    .truncate()
                    .cursor_pointer()
                    .child(group_name)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_group_edit(
                            GroupEdit::Rename(rename_group_id.clone()),
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(count.to_string()),
            )
            .child(actions)
            .into_any_element()
    }

    pub(super) fn group_delete_modal(&self, cx: &mut Context<Self>) -> Div {
        let mut p = self.palette();
        p.elevated.a = 1.;
        let Some(group) =
            self.model.library.groups().into_iter().find(|group| {
                self.groups_flow.delete_pending.as_deref() == Some(group.id.as_str())
            })
        else {
            return div();
        };
        let count = self
            .model
            .skills
            .iter()
            .filter(|skill| skill.record.group_id.as_deref() == Some(group.id.as_str()))
            .count();
        let delete_skills = self.groups_flow.delete_skills;
        div()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .child(
                div()
                    .p(px(20.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.tr("删除分组", "Delete group")),
                            )
                            .child(
                                self.bordered_icon_button(
                                    "close-delete-group",
                                    "icons/x.svg",
                                    p.text,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(12.))
                            .text_size(px(14.))
                            .line_height(relative(1.55))
                            .child(if self.uses_english() {
                                format!("Delete “{}”?", group.name)
                            } else {
                                format!("确定删除「{}」吗？", group.name)
                            }),
                    )
                    .child(
                        div()
                            .mt(px(6.))
                            .text_size(px(13.))
                            .text_color(p.secondary)
                            .child(self.tr(
                                "分组中的技能会保留并移到未分组。",
                                "Skills will be kept and moved to Ungrouped.",
                            )),
                    )
                    .when(count > 0, |body| {
                        body.child(
                            div()
                                .id("delete-group-skills-option")
                                .mt(px(16.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .cursor_pointer()
                                .child(
                                    Checkbox::new("delete-group-skills-checkbox")
                                        .checked(delete_skills),
                                )
                                .child(div().text_size(px(13.)).child(if self.uses_english() {
                                    format!("Also delete the {count} Skills in this group")
                                } else {
                                    format!("同时删除分组中的 {} 个技能", count)
                                }))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.groups_flow.delete_skills =
                                        !this.groups_flow.delete_skills;
                                    this.notify_dialog(cx);
                                })),
                        )
                    })
                    .when(delete_skills, |body| {
                        body.child(
                            div()
                                .mt(px(8.))
                                .text_size(px(12.))
                                .text_color(p.danger)
                                .child(self.tr(
                                    "技能删除后无法撤销。",
                                    "Deleting Skills cannot be undone.",
                                )),
                        )
                    })
                    .when_some(self.tags_flow.error.clone(), |body, error| {
                        body.child(
                            div()
                                .mt(px(8.))
                                .text_size(px(12.))
                                .text_color(p.danger)
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(60.))
                    .px(px(20.))
                    .border_t_1()
                    .border_color(p.border)
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("cancel-delete-group")
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
                                cx.listener(|this, _, _, cx| this.close_dialog(cx)),
                            ),
                    )
                    .child(
                        div()
                            .id("confirm-delete-group")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .border_1()
                            .border_color(p.border)
                            .bg(p.danger)
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(14.))
                            .text_color(p.on_accent)
                            .hover(move |button| button.opacity(0.82))
                            .child(if delete_skills {
                                self.tr("删除分组及技能", "Delete group and Skills")
                            } else {
                                self.tr("删除分组", "Delete group")
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.delete_skill_group(group.id.clone(), delete_skills, cx);
                                }),
                            ),
                    ),
            )
    }

    pub(super) fn group_management_modal(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let groups = self.model.library.groups();
        let mut rows = div().mt(px(12.)).flex().flex_col().gap(px(2.));
        if self.groups_flow.edit == Some(GroupEdit::Create) {
            rows = rows.child(self.group_inline_editor("new-group-row", cx));
        }
        if groups.is_empty() && self.groups_flow.edit != Some(GroupEdit::Create) {
            rows = rows.child(
                div()
                    .h(px(54.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(p.muted)
                    .child("还没有分组"),
            );
        }
        for group in groups {
            rows = rows.child(self.group_management_row(group, cx));
        }
        rows = rows.child(
            div()
                .id("new-group-at-end")
                .mt(px(6.))
                .h(px(CONTROL_HEIGHT))
                .px(px(8.))
                .rounded(px(RADIUS_CONTROL))
                .flex()
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(p.secondary)
                .hover(move |row| row.bg(p.hover))
                .child(Self::icon("icons/plus.svg", 13., p.secondary))
                .child("新建分组")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.start_group_edit(GroupEdit::Create, window, cx);
                })),
        );
        div()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .id("group-manager-scroll")
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(18.))
                    .max_h(px(600.))
                    .overflow_y_scroll()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.groups_flow.edit.is_some() {
                                this.groups_flow.edit = None;
                                this.tags_flow.error = None;
                                this.notify_dialog(cx);
                            }
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("分组管理"),
                            )
                            .child(div().flex_1())
                            .child(div().w(px(6.)))
                            .child(
                                div()
                                    .id("close-groups-modal")
                                    .size(px(CONTROL_HEIGHT))
                                    .rounded(px(RADIUS_CONTROL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .bg(p.raised)
                                    .hover(move |button| button.bg(p.hover))
                                    .child(Self::icon("icons/x.svg", 14., p.text))
                                    .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                            ),
                    )
                    .child(rows)
                    .when_some(self.tags_flow.error.clone(), |panel, error| {
                        panel.child(
                            div()
                                .mt(px(8.))
                                .text_size(px(12.))
                                .text_color(p.danger)
                                .child(error),
                        )
                    }),
            )
    }

    pub(super) fn tag_filter_control(&self, scope: TagScope, cx: &mut Context<Self>) -> Popover {
        let p = self.palette();
        let english = self.uses_english();
        let selected = self.tag_filter_for(scope);
        let tag_state = self.tags_for(scope);
        let tags = tag_state
            .tags()
            .iter()
            .map(|tag| {
                (
                    tag.id,
                    tag.name.clone(),
                    tag.parent,
                    tag_state.count(tag.id),
                )
            })
            .collect::<Vec<_>>();
        let scope_prefix = match scope {
            TagScope::Skills => "skill",
            TagScope::Projects => "project",
        };
        let all_label = match (scope, english) {
            (TagScope::Skills, true) => "All Skills",
            (TagScope::Projects, true) => "All Projects",
            (TagScope::Skills, false) => "全部技能",
            (TagScope::Projects, false) => "全部项目",
        };
        let app = cx.entity().downgrade();
        Popover::new(ElementId::Name(
            format!("{scope_prefix}-tag-filter-menu").into(),
        ))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new(ElementId::Name(
                format!("{scope_prefix}-tag-filter-trigger").into(),
            ))
            .small()
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(rgba(0x00000000).into())
                    .foreground(
                        if selected.is_some() {
                            p.accent
                        } else {
                            p.muted
                        }
                        .into(),
                    )
                    .hover(p.hover.into())
                    .active(p.selected.into()),
            )
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .child(Self::icon(
                "icons/hash.svg",
                15.,
                if selected.is_some() {
                    p.accent
                } else {
                    p.muted
                },
            )),
        )
        .content(move |_, _, popover_cx| {
            let mut menu = div()
                .w(px(220.))
                .p(px(4.))
                .rounded(px(RADIUS_MENU))
                .border_1()
                .border_color(p.border)
                .bg(p.elevated)
                .shadow_lg();
            let all_app = app.clone();
            menu = menu.child(
                div()
                    .id(ElementId::Name(
                        format!("{scope_prefix}-tag-filter-all").into(),
                    ))
                    .h(px(29.))
                    .px(px(8.))
                    .rounded(px(RADIUS_CONTROL))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(if selected.is_none() {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |row| row.bg(p.hover))
                    .text_size(px(13.))
                    .child(all_label)
                    .on_click(popover_cx.listener(move |_, _, _, cx| {
                        let _ = all_app.update(cx, |this, cx| {
                            this.set_tag_filter(scope, None);
                            cx.notify();
                        });
                        cx.emit(DismissEvent);
                    })),
            );
            for (id, name, _parent, count) in tags.iter().filter(|tag| tag.2.is_none()) {
                let root_app = app.clone();
                let root_id = *id;
                menu = menu.child(
                    div()
                        .id(ElementId::Name(
                            format!("{scope_prefix}-tag-filter-{id}").into(),
                        ))
                        .h(px(29.))
                        .px(px(8.))
                        .rounded(px(RADIUS_CONTROL))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .bg(if selected == Some(*id) {
                            p.selected
                        } else {
                            rgba(0x00000000)
                        })
                        .hover(move |row| row.bg(p.hover))
                        .child(Self::icon("icons/chevron-down.svg", 14., p.muted))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_size(px(13.))
                                .truncate()
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .font_family(MONO)
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(count.to_string()),
                        )
                        .on_click(popover_cx.listener(move |_, _, _, cx| {
                            let _ = root_app.update(cx, |this, cx| {
                                this.set_tag_filter(scope, Some(root_id));
                                cx.notify();
                            });
                            cx.emit(DismissEvent);
                        })),
                );
                for (child_id, child_name, _, child_count) in
                    tags.iter().filter(|tag| tag.2 == Some(*id))
                {
                    let child_app = app.clone();
                    let child_id = *child_id;
                    menu = menu.child(
                        div()
                            .id(ElementId::Name(
                                format!("{scope_prefix}-tag-filter-{child_id}").into(),
                            ))
                            .h(px(29.))
                            .pl(px(28.))
                            .pr(px(8.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(if selected == Some(child_id) {
                                p.selected
                            } else {
                                rgba(0x00000000)
                            })
                            .hover(move |row| row.bg(p.hover))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .text_size(px(13.))
                                    .truncate()
                                    .child(child_name.clone()),
                            )
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(child_count.to_string()),
                            )
                            .on_click(popover_cx.listener(move |_, _, _, cx| {
                                let _ = child_app.update(cx, |this, cx| {
                                    this.set_tag_filter(scope, Some(child_id));
                                    cx.notify();
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
            }
            menu
        })
    }

    pub(super) fn skills_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let query = self
            .skills_view
            .skill_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let visible_skills = self
            .model
            .skills
            .iter()
            .filter(|skill| {
                let matches_query = query.is_empty()
                    || skill.record.name.to_lowercase().contains(&query)
                    || skill.record.description.to_lowercase().contains(&query)
                    || skill.record.origin.label().to_lowercase().contains(&query);
                let matches_tag = self.tags_flow.selected_skill_filter.is_none_or(|tag| {
                    self.tags_flow
                        .skills
                        .matches_filter(skill_storage_name(skill), tag)
                });
                matches_query && matches_tag
            })
            .collect::<Vec<_>>();
        let visible_count = visible_skills.len();
        let configured_groups = self.model.library.groups();
        let group_ids = configured_groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<HashSet<_>>();
        let mut installable_group_skills =
            installable_skills_by_group(&self.model.skills, &group_ids);
        let mut grouped = BTreeMap::<String, Vec<&SkillSummary>>::new();
        let mut pinned = Vec::new();
        let mut ungrouped = Vec::new();
        for skill in visible_skills {
            if skill.record.origin.is_builtin() {
                pinned.push(skill);
            } else if let Some(group_id) = skill.record.group_id.as_deref()
                && group_ids.contains(group_id)
            {
                grouped.entry(group_id.to_string()).or_default().push(skill);
            } else {
                ungrouped.push(skill);
            }
        }
        for skills in grouped.values_mut() {
            skills.sort_by(|left, right| {
                right
                    .record
                    .last_operated_at
                    .cmp(&left.record.last_operated_at)
                    .then_with(|| left.record.name.cmp(&right.record.name))
            });
        }
        ungrouped.sort_by(|left, right| {
            right
                .record
                .last_operated_at
                .cmp(&left.record.last_operated_at)
                .then_with(|| left.record.name.cmp(&right.record.name))
        });
        let mut selection_order = Vec::with_capacity(visible_count);
        selection_order.extend(
            pinned
                .iter()
                .map(|skill| skill_storage_name(skill).to_string()),
        );
        for group in &configured_groups {
            let collapsed =
                query.is_empty() && self.skills_view.collapsed_groups.contains(&group.id);
            if !collapsed {
                if let Some(skills) = grouped.get(&group.id) {
                    selection_order.extend(
                        skills
                            .iter()
                            .map(|skill| skill_storage_name(skill).to_string()),
                    );
                }
            }
        }
        selection_order.extend(
            ungrouped
                .iter()
                .map(|skill| skill_storage_name(skill).to_string()),
        );
        let selection_count = self.selected_skill_keys().len();
        let selection_order = Arc::<[String]>::from(selection_order);
        let visible_selection_order = Arc::clone(&selection_order);
        let mut list = div()
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
                    "skills-panel-window-drag",
                    self.tr("技能", "Skills"),
                    self.model.skills.len(),
                    cx,
                )
                .child(self.skills_manage_control(cx))
                .child(div().w(px(8.)).flex_none())
                .child(
                    self.labeled_icon_button(
                        "add-skill",
                        "icons/plus.svg",
                        p.text,
                        self.tr("添加技能", "Add Skill"),
                        cx,
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.open_add_dialog(window, cx);
                        }),
                    ),
                ),
            )
            .child(
                div().px(px(10.)).py(px(8.)).child(
                    Input::new(&self.skills_view.skill_search)
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
                            .selected_skill_filter
                            .and_then(|tag| self.tags_flow.skills.path(tag))
                            .map(|path| format!("#{path}  {visible_count}"))
                            .unwrap_or_else(|| {
                                format!("{}  {}", self.tr("全部", "All"), visible_count)
                            }),
                    )
                    .child(div().flex_1())
                    .child(self.tag_filter_control(TagScope::Skills, cx)),
            )
            .when(self.skills_view.selection.is_multiple(), |list| {
                let visible_selection_order = visible_selection_order.clone();
                list.child(
                    div()
                        .px(px(16.))
                        .pb(px(6.))
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .text_color(p.secondary)
                        .child(if self.uses_english() {
                            format!("{selection_count} Skills selected")
                        } else {
                            format!("已选 {} 个技能", selection_count)
                        })
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("select-all-visible-skills")
                                .px(px(5.))
                                .cursor_pointer()
                                .hover(move |button| button.text_color(p.text))
                                .child(self.tr("全选", "All"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_all_visible_skills(
                                        visible_selection_order.as_ref(),
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            div()
                                .id("clear-skill-selection")
                                .px(px(5.))
                                .cursor_pointer()
                                .hover(move |button| button.text_color(p.text))
                                .child(self.tr("清除", "Clear"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clear_skill_selection(cx);
                                })),
                        ),
                )
            });
        let mut rows = div()
            .id("skill-groups-scroll")
            .flex_1()
            .min_h_0()
            .px(px(8.))
            .pb(px(8.))
            .overflow_y_scroll();
        for skill in pinned {
            rows = rows.child(self.skill_list_row(skill, false, Arc::clone(&selection_order), cx));
        }
        for group in configured_groups {
            let group_id = group.id.clone();
            let group_label = group.name.clone();
            let skills = grouped.remove(&group_id).unwrap_or_default();
            let collapsed =
                query.is_empty() && self.skills_view.collapsed_groups.contains(&group_id);
            let group_count = skills.len();
            let count_label = group_count.to_string();
            let toggle_key = group_id.clone();
            let drop_group_id = group_id.clone();
            let context_app = cx.entity().downgrade();
            let rename_group_id = group_id.clone();
            let delete_group_id = group_id.clone();
            let delete_group_menu_color = p.danger;
            let install_group_skills = installable_group_skills
                .remove(&group_id)
                .unwrap_or_default();
            let install_group_count = install_group_skills.len();
            let install_group_label = if self.uses_english() {
                format!("Install {install_group_count} Skills")
            } else {
                format!("安装分组内 {install_group_count} 个技能")
            };
            rows = rows.child(
                div()
                    .id(ElementId::Name(format!("skill-group-{group_id}").into()))
                    .h(px(ROW_HEIGHT))
                    .px(px(8.))
                    .mt(px(3.))
                    .rounded(px(RADIUS_LIST_ROW))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_color(p.secondary)
                    .hover(move |row| row.bg(p.hover))
                    .child(Self::icon(
                        if collapsed {
                            "icons/chevron-right.svg"
                        } else {
                            "icons/chevron-down.svg"
                        },
                        14.,
                        p.muted,
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .font_family(MONO)
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(group_label),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(count_label),
                    )
                    .drag_over::<SkillDrag>(move |style, _, _, _| style.bg(p.selected))
                    .on_drop(cx.listener(move |this, drag: &SkillDrag, _, cx| {
                        this.assign_skill_group(drag.name.clone(), Some(drop_group_id.clone()), cx);
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.skills_view.collapsed_groups.remove(&toggle_key) {
                            this.skills_view.collapsed_groups.insert(toggle_key.clone());
                        }
                        cx.notify();
                    }))
                    .context_menu(move |menu, _, _| {
                        let install_app = context_app.clone();
                        let rename_app = context_app.clone();
                        let delete_app = context_app.clone();
                        let install_group_skills = install_group_skills.clone();
                        let rename_group_id = rename_group_id.clone();
                        let delete_group_id = delete_group_id.clone();
                        let delete_menu_item =
                            Self::danger_menu_item("删除分组", delete_group_menu_color).on_click(
                                move |_, _, cx| {
                                    let _ = delete_app.update(cx, |this, cx| {
                                        this.open_group_delete_dialog(delete_group_id.clone(), cx);
                                    });
                                },
                            );
                        menu.min_w(px(190.))
                            .when(!install_group_skills.is_empty(), |menu| {
                                menu.item(
                                    PopupMenuItem::new(install_group_label.clone())
                                        .icon(Icon::default().path("icons/download.svg"))
                                        .on_click(move |_, window, cx| {
                                            let _ = install_app.update(cx, |this, cx| {
                                                let primary = this
                                                    .skills_view
                                                    .selection
                                                    .select_all(&install_group_skills);
                                                this.set_detail_selection(primary);
                                                this.open_install_dialog(window, cx);
                                            });
                                        }),
                                )
                            })
                            .item(
                                PopupMenuItem::new("重命名")
                                    .icon(Icon::default().path("icons/pencil.svg"))
                                    .on_click(move |_, window, cx| {
                                        let _ = rename_app.update(cx, |this, cx| {
                                            this.open_group_dialog(cx);
                                            this.start_group_edit(
                                                GroupEdit::Rename(rename_group_id.clone()),
                                                window,
                                                cx,
                                            );
                                        });
                                    }),
                            )
                            .item(delete_menu_item)
                    }),
            );
            if collapsed {
                continue;
            }
            for skill in skills {
                rows =
                    rows.child(self.skill_list_row(skill, true, Arc::clone(&selection_order), cx));
            }
        }
        for skill in ungrouped {
            rows = rows.child(self.skill_list_row(skill, false, Arc::clone(&selection_order), cx));
        }
        list = list.child(rows);
        layout::content(
            &self.shell.content_layout,
            self.content_pane_width(window, cx),
            list,
            self.skill_detail(window, cx),
        )
    }

    pub(super) fn skill_tag_controls(&self, skill_name: &str, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let english = self.uses_english();
        let app = cx.entity().downgrade();
        let assigned = self
            .tags_flow
            .skills
            .assigned_tags(skill_name)
            .into_iter()
            .filter_map(|tag| {
                self.tags_flow
                    .skills
                    .path(tag.id)
                    .map(|path| (tag.id, path))
            })
            .collect::<Vec<_>>();
        let tag_rows = self
            .tags_flow
            .skills
            .tags()
            .iter()
            .map(|tag| {
                (
                    tag.id,
                    tag.name.clone(),
                    tag.parent,
                    self.tags_flow.skills.is_assigned(skill_name, tag.id),
                )
            })
            .collect::<Vec<_>>();
        let mut labels = div()
            .mt(px(12.))
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_wrap();
        for (tag_id, path) in assigned {
            labels = labels.child(
                div()
                    .id(ElementId::Name(format!("skill-tag-{tag_id}").into()))
                    .px(px(2.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(p.secondary)
                    .hover(move |label| label.bg(p.hover).text_color(p.text))
                    .child(format!("#{path}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tags_flow.selected_skill_filter = Some(tag_id);
                        cx.notify();
                    })),
            );
        }
        let skill = skill_name.to_string();
        let has_assigned = !self.tags_flow.skills.assigned_tags(skill_name).is_empty();
        let editor = Popover::new(ElementId::Name(
            format!("skill-tags-menu-{skill_name}").into(),
        ))
        .appearance(false)
        .anchor(Anchor::TopLeft)
        .trigger(
            Button::new(ElementId::Name(
                format!("skill-tags-trigger-{skill_name}").into(),
            ))
            .small()
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(rgba(0x00000000).into())
                    .foreground(p.muted.into())
                    .hover(p.hover.into())
                    .active(p.selected.into()),
            )
            .h(px(CONTROL_HEIGHT))
            .px(px(6.))
            .rounded(px(RADIUS_CONTROL))
            .child(if has_assigned {
                self.tr("编辑", "Edit")
            } else {
                self.tr("＋ 添加标签", "+ Add tag")
            }),
        )
        .content(move |_, _, popover_cx| {
            let mut menu = div()
                .w(px(240.))
                .p(px(4.))
                .rounded(px(RADIUS_MENU))
                .border_1()
                .border_color(p.border_strong)
                .bg(p.elevated)
                .shadow_lg();
            if tag_rows.is_empty() {
                menu = menu.child(
                    div()
                        .px(px(8.))
                        .py(px(10.))
                        .text_size(px(13.))
                        .text_color(p.muted)
                        .child(if english {
                            "No tags yet"
                        } else {
                            "还没有标签"
                        }),
                );
            }
            for (id, name, parent, checked) in &tag_rows {
                let tag_app = app.clone();
                let tag_skill = skill.clone();
                let tag_id = *id;
                menu = menu.child(
                    div()
                        .id(ElementId::Name(format!("assign-tag-{tag_id}").into()))
                        .h(px(29.))
                        .pl(px(if parent.is_some() { 28. } else { 8. }))
                        .pr(px(8.))
                        .rounded(px(RADIUS_CONTROL))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(move |row| row.bg(p.hover))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_size(px(13.))
                                .truncate()
                                .child(format!("#{}", name)),
                        )
                        .when(*checked, |row| {
                            row.child(Self::icon("icons/check.svg", 14., p.text))
                        })
                        .on_click(popover_cx.listener(move |_, _, _, cx| {
                            let _ = tag_app.update(cx, |this, app_cx| {
                                this.tags_flow.skills.toggle_assignment(&tag_skill, tag_id);
                                this.persist_tags();
                                app_cx.notify();
                            });
                            cx.notify();
                        })),
                );
            }
            let new_tag_app = app.clone();
            menu.child(
                div()
                    .mt(px(4.))
                    .pt(px(4.))
                    .border_t_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .id("new-tag-from-skill")
                            .h(px(29.))
                            .px(px(8.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(p.secondary)
                            .hover(move |row| row.bg(p.hover))
                            .child(if english {
                                "+ New tag"
                            } else {
                                "＋ 新建标签"
                            })
                            .on_click(popover_cx.listener(move |_, _, _, cx| {
                                let _ = new_tag_app.update(cx, |this, cx| {
                                    this.open_tag_dialog(TagScope::Skills, cx)
                                });
                                cx.emit(DismissEvent);
                            })),
                    ),
            )
        });
        labels.child(editor)
    }

    pub(super) fn multi_skill_detail(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let selected_names = self
            .model
            .skills
            .iter()
            .filter(|skill| {
                self.skills_view
                    .selection
                    .contains(skill_storage_name(skill))
            })
            .map(|skill| skill.record.name.clone())
            .collect::<Vec<_>>();
        let selection_count = selected_names.len();
        let preview_count = 6;
        let mut names = div().mt(px(20.)).flex().flex_col().gap(px(4.));
        for name in selected_names.iter().take(preview_count) {
            names = names.child(
                div()
                    .h(px(30.))
                    .px(px(10.))
                    .rounded(px(RADIUS_CONTROL))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .bg(p.raised)
                    .child(Self::icon("icons/package.svg", 14., p.secondary))
                    .child(
                        div()
                            .min_w_0()
                            .font_family(MONO)
                            .text_size(px(13.))
                            .truncate()
                            .child(name.clone()),
                    ),
            );
        }
        if selection_count > preview_count {
            names = names.child(
                div()
                    .mt(px(4.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(if self.uses_english() {
                        format!("and {} more", selection_count - preview_count)
                    } else {
                        format!("还有 {} 个", selection_count - preview_count)
                    }),
            );
        }
        div()
            .relative()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .px(px(24.))
            .pt(px(28.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .size(px(40.))
                            .rounded(px(RADIUS_CONTROL))
                            .bg(p.raised)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Self::icon("icons/package.svg", 20., p.secondary)),
                    )
                    .child(
                        div()
                            .ml(px(14.))
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if self.uses_english() {
                                        format!("{selection_count} Skills selected")
                                    } else {
                                        format!("已选 {} 个技能", selection_count)
                                    }),
                            )
                            .child(
                                div()
                                    .mt(px(4.))
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(self.tr(
                                        "批量操作将应用到全部选中项",
                                        "Batch actions apply to all selected Skills",
                                    )),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        self.labeled_icon_button(
                            "install-selected-skills",
                            "icons/download.svg",
                            p.text,
                            self.tr("安装技能", "Install Skills"),
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.open_install_dialog(window, cx);
                            }),
                        ),
                    ),
            )
            .child(names)
            .child(self.window_drag_strip("multi-skill-detail-window-drag", 24., cx))
    }

    pub(super) fn skill_detail(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        if self.skills_view.selection.is_multiple() && self.skills_view.selection.len() > 1 {
            return self.multi_skill_detail(cx);
        }
        let Some(skill) = self.selected_skill() else {
            return div()
                .relative()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.muted)
                .child(self.tr(
                    "添加第一个技能开始使用 Kitter",
                    "Add your first Skill to get started",
                ))
                .child(self.window_drag_strip("empty-skill-detail-window-drag", 24., cx));
        };
        let storage_name = skill_storage_name(skill).to_string();
        let skill_path = skill.path.clone();
        let mut actions = div().flex().gap(px(8.));
        if skill.record.update_available {
            let update_name = storage_name.clone();
            actions = actions.child(
                self.labeled_icon_button(
                    "update-skill",
                    "icons/rotate-cw.svg",
                    p.secondary,
                    self.tr("更新", "Update"),
                    cx,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.update_skill(update_name.clone(), cx);
                    }),
                ),
            );
        }
        let delete_storage_name = storage_name.clone();
        actions = actions
            .child(
                self.danger_icon_button("delete-skill", "icons/trash.svg", cx)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.delete_flow.selected.clear();
                            this.delete_flow.confirmation =
                                Some(DeleteConfirmation::LibrarySkills {
                                    skills: vec![(delete_storage_name.clone(), skill_path.clone())],
                                });
                            this.open_delete_dialog(window, cx);
                        }),
                    ),
            )
            .child(
                self.primary_icon_button(
                    "install-skill",
                    "icons/download.svg",
                    self.tr("安装技能", "Install Skill"),
                    cx,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if this.projects_view.open_project.is_none() {
                            this.projects_view.open_project =
                                this.model.library.config.project_paths().first().cloned();
                        }
                        this.open_install_dialog(window, cx);
                    }),
                ),
            );
        let description = if skill.record.description.is_empty() {
            self.tr(
                "这个技能暂时没有描述。",
                "This Skill does not have a description yet.",
            )
            .to_string()
        } else {
            skill.record.description.clone()
        };
        let available_width = self.content_pane_width(window, cx);
        let list_width = f32::from(
            self.shell
                .content_layout
                .read(cx)
                .sizes()
                .first()
                .copied()
                .unwrap_or(px(layout::LIST_WIDTH)),
        )
        .clamp(220., (available_width - 380.).clamp(220., 420.));
        let description_width =
            (available_width - list_width - 48.).clamp(1., DESCRIPTION_MAX_WIDTH);
        let description_lines = window
            .text_system()
            .shape_text(
                description.clone().into(),
                px(14.),
                &[TextRun {
                    len: description.len(),
                    font: font(FONT_UI),
                    color: p.secondary.into(),
                    ..Default::default()
                }],
                Some(px(description_width)),
                None,
            )
            .map(|lines| {
                lines
                    .iter()
                    .map(|line| line.wrap_boundaries.len() + 1)
                    .sum()
            })
            .unwrap_or(1);
        let can_expand_description = description_lines > DESCRIPTION_COLLAPSED_LINES;
        let description_expanded = can_expand_description
            && self.skills_view.expanded_description.as_deref() == Some(skill.record.name.as_str());
        let mut description_text = div()
            .relative()
            .font_family(FONT_UI)
            .text_size(px(14.))
            .line_height(relative(1.5))
            .text_color(p.secondary)
            .child(self.selectable_text("skill-detail-description", 102, description, window, cx));
        if can_expand_description && !description_expanded {
            description_text = description_text
                .line_clamp(DESCRIPTION_COLLAPSED_LINES)
                .text_ellipsis()
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .right(px(0.))
                        .bottom(px(0.))
                        .h(px(18.))
                        .bg(linear_gradient(
                            0.,
                            linear_color_stop(p.base, 0.),
                            linear_color_stop(p.base.opacity(0.), 1.),
                        )),
                );
        }
        let mut description_section = div()
            .mt(px(16.))
            .max_w(px(DESCRIPTION_MAX_WIDTH))
            .child(description_text);
        if can_expand_description {
            let description_name = skill.record.name.clone();
            let toggle_label = if description_expanded {
                self.tr("收起", "Less")
            } else {
                self.tr("展开", "More")
            };
            description_section = description_section.child(
                div().w_full().mt(px(4.)).flex().items_center().child(
                    div()
                        .id("toggle-skill-description")
                        .h(px(24.))
                        .px(px(6.))
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .cursor_pointer()
                        .font_family(FONT_UI)
                        .text_size(px(12.))
                        .text_color(p.secondary)
                        .hover(move |button| button.bg(p.hover).text_color(p.text))
                        .child(toggle_label)
                        .child(Self::icon(
                            if description_expanded {
                                "icons/chevron-up.svg"
                            } else {
                                "icons/chevron-down.svg"
                            },
                            12.,
                            p.secondary,
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if this.skills_view.expanded_description.as_deref()
                                    == Some(description_name.as_str())
                                {
                                    this.skills_view.expanded_description = None;
                                } else {
                                    this.skills_view.expanded_description =
                                        Some(description_name.clone());
                                }
                                cx.notify();
                            }),
                        ),
                ),
            );
        }
        let header = div()
            .px(px(24.))
            .pt(px(24.))
            .pb(px(16.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .size(px(40.))
                            .rounded(px(RADIUS_CONTROL))
                            .bg(p.raised)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Self::icon("icons/package.svg", 20., p.secondary)),
                    )
                    .child(
                        div()
                            .ml(px(14.))
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .font_family(MONO)
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.selectable_text(
                                        "skill-detail-name",
                                        100,
                                        skill.record.name.clone(),
                                        window,
                                        cx,
                                    ))
                                    .when(skill.manual_only, |title| {
                                        title.child(self.manual_skill_badge())
                                    }),
                            )
                            .child(
                                div()
                                    .mt(px(4.))
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(self.selectable_text(
                                        "skill-detail-origin",
                                        101,
                                        skill.record.origin.label(),
                                        window,
                                        cx,
                                    )),
                            ),
                    )
                    .child(div().flex_1())
                    .child(actions),
            )
            .child(description_section)
            .child(self.skill_tag_controls(&storage_name, cx));
        let tabs = div()
            .h(px(40.))
            .px(px(24.))
            .flex()
            .items_center()
            .gap(px(2.))
            .border_b_1()
            .border_color(p.border)
            .child(self.tab_button(
                "tab-installs",
                self.tr("安装情况", "Installs"),
                DetailTab::Installs,
                cx,
            ))
            .child(self.tab_button(
                "tab-content",
                self.tr("内容", "Content"),
                DetailTab::Content,
                cx,
            ));
        div()
            .relative()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(tabs)
            .child(if self.skills_view.tab == DetailTab::Installs {
                self.installs_tab(skill, window, cx)
            } else {
                self.content_tab(skill, window, cx)
            })
            .child(self.window_drag_strip("skill-detail-window-drag", 24., cx))
    }

    pub(super) fn tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: DetailTab,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let active = self.skills_view.tab == tab;
        div()
            .id(id)
            .h(px(CONTROL_HEIGHT))
            .px(px(8.))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(if active { p.text } else { p.muted })
            .bg(if active { p.selected } else { rgba(0x00000000) })
            .hover(move |tab| tab.bg(p.hover))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.skills_view.tab = tab;
                cx.notify();
            }))
    }

    pub(super) fn skill_installations_at(
        &self,
        skill: &SkillSummary,
        root: &PathBuf,
        cx: &mut Context<Self>,
    ) -> Vec<ProjectSkillInstallation> {
        if !self
            .projects_view
            .project_snapshots
            .borrow()
            .contains_key(root)
        {
            self.project_snapshot(root, cx);
        }
        self.projects_view
            .project_snapshots
            .borrow()
            .get(root)
            .into_iter()
            .flat_map(|snapshot| snapshot.iter())
            .filter(|installed| installed.name == skill.record.name)
            .flat_map(|installed| installed.installations.iter())
            .filter(|installation| same_file(&installation.path, &skill.path))
            .cloned()
            .collect()
    }

    pub(super) fn skill_installed_at(
        &self,
        skill: &SkillSummary,
        root: &PathBuf,
        cx: &mut Context<Self>,
    ) -> bool {
        !self.skill_installations_at(skill, root, cx).is_empty()
    }

    pub(super) fn installs_tab(
        &self,
        skill: &SkillSummary,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let mut body = div().p(px(24.)).flex_1().overflow_hidden();
        let home = dirs::home_dir();
        let roots = home
            .iter()
            .cloned()
            .chain(
                self.model
                    .library
                    .config
                    .project_paths()
                    .into_iter()
                    .filter(|path| Some(path) != home.as_ref()),
            )
            .collect::<Vec<_>>();
        self.request_project_snapshots(&roots, cx);
        let projects = roots
            .into_iter()
            .filter(|path| self.skill_installed_at(skill, path, cx))
            .collect::<Vec<_>>();
        if projects.is_empty() {
            body = body.child(
                div()
                    .mt(px(34.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_color(p.muted)
                    .child(Self::icon("icons/package.svg", 28., p.muted))
                    .child(
                        div()
                            .mt(px(10.))
                            .text_size(px(12.))
                            .child(self.tr("尚未安装", "Not installed yet")),
                    ),
            );
        } else {
            body = body.child(
                div()
                    .mb(px(10.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(if self.uses_english() {
                        format!("Installed in {} location(s)", projects.len())
                    } else {
                        format!("已安装到 {} 个位置", projects.len())
                    }),
            );
            for (index, path) in projects.into_iter().enumerate() {
                let popover_id = format!("agents:{}:{}", skill.record.name, path.display());
                let project_name = if home.as_ref() == Some(&path) {
                    self.tr("全局", "Global").to_string()
                } else {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                };
                let project_path = display_path(&path);
                let installations = self.skill_installations_at(skill, &path, cx);
                let targets = installations
                    .iter()
                    .map(|installation| installation.target)
                    .collect::<Vec<_>>();
                let project_for_delete = path.clone();
                let skill_for_delete = ProjectSkill {
                    name: skill.record.name.clone(),
                    installations,
                };
                body = body.child(
                    div()
                        .h(px(64.))
                        .px(px(14.))
                        .border_b_1()
                        .border_color(p.border)
                        .flex()
                        .items_center()
                        .child(Self::icon("icons/folder.svg", 16., p.secondary))
                        .child(
                            div()
                                .ml(px(11.))
                                .min_w_0()
                                .flex_1()
                                .child(div().font_family(MONO).text_size(px(13.)).child(
                                    self.selectable_text(
                                        format!("install-project-name-{index}"),
                                        110 + index as u64 * 2,
                                        project_name,
                                        window,
                                        cx,
                                    ),
                                ))
                                .child(
                                    div()
                                        .mt(px(3.))
                                        .font_family(MONO)
                                        .text_size(px(12.))
                                        .text_color(p.muted)
                                        .child(self.selectable_text(
                                            format!("install-project-path-{index}"),
                                            111 + index as u64 * 2,
                                            project_path,
                                            window,
                                            cx,
                                        )),
                                ),
                        )
                        .child(self.agent_badges(popover_id, &targets, false, cx))
                        .child(
                            self.danger_icon_button(
                                ElementId::Name(format!("remove-install-project-{index}").into()),
                                "icons/trash.svg",
                                cx,
                            )
                            .ml(px(10.))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.delete_flow.selected =
                                        unique_installation_paths(&skill_for_delete.installations)
                                            .into_iter()
                                            .collect();
                                    this.delete_flow.confirmation =
                                        Some(DeleteConfirmation::ProjectSkill {
                                            project: project_for_delete.clone(),
                                            skill: skill_for_delete.clone(),
                                        });
                                    this.open_delete_dialog(window, cx);
                                }),
                            ),
                        ),
                );
            }
        }
        body
    }

    pub(super) fn content_tab(
        &self,
        skill: &SkillSummary,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let snapshot = self.content_snapshot(skill);
        let files = &snapshot.files;
        let mut directories = Vec::<PathBuf>::new();
        for file in files {
            let mut parent = file.parent();
            while let Some(path) = parent {
                if !path.as_os_str().is_empty() && !directories.iter().any(|item| item == path) {
                    directories.push(path.to_path_buf());
                }
                parent = path.parent();
            }
        }
        directories.sort();
        let mut entries = directories
            .into_iter()
            .map(|path| (path, true))
            .chain(files.iter().cloned().map(|path| (path, false)))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.0
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.0.to_string_lossy().to_lowercase())
                .then_with(|| right.1.cmp(&left.1))
        });
        let mut tree = div()
            .id("content-tree-scroll")
            .w(px(220.))
            .h_full()
            .flex_none()
            .bg(p.surface)
            .py(px(8.))
            .overflow_y_scroll();
        for (file, is_directory) in entries {
            if is_directory {
                let hidden_by_parent = file
                    .ancestors()
                    .skip(1)
                    .filter(|path| !path.as_os_str().is_empty())
                    .any(|path| {
                        self.skills_view
                            .collapsed_content_directories
                            .contains(path)
                    });
                if hidden_by_parent {
                    continue;
                }
                let depth = file.components().count().saturating_sub(1) as f32;
                let collapsed = self
                    .skills_view
                    .collapsed_content_directories
                    .contains(&file);
                let directory_for_click = file.clone();
                tree = tree.child(
                    div()
                        .id(ElementId::Name(
                            format!("directory-{}", file.display()).into(),
                        ))
                        .h(px(ROW_HEIGHT))
                        .mx(px(6.))
                        .pl(px(3. + depth * 14.))
                        .pr(px(9.))
                        .rounded(px(RADIUS_LIST_ROW))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_color(p.secondary)
                        .hover(move |row| row.bg(p.hover))
                        .child(
                            div()
                                .size(px(14.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Self::icon(
                                    if collapsed {
                                        "icons/chevron-right.svg"
                                    } else {
                                        "icons/chevron-down.svg"
                                    },
                                    14.,
                                    p.muted,
                                )),
                        )
                        .child(
                            div()
                                .size(px(18.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Self::icon(
                                    if collapsed {
                                        "icons/folder.svg"
                                    } else {
                                        "icons/folder-open.svg"
                                    },
                                    15.,
                                    p.secondary,
                                )),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .font_family(MONO)
                                .text_size(px(12.))
                                .truncate()
                                .child(
                                    file.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this
                                .skills_view
                                .collapsed_content_directories
                                .remove(&directory_for_click)
                            {
                                this.skills_view
                                    .collapsed_content_directories
                                    .insert(directory_for_click.clone());
                            }
                            cx.notify();
                        })),
                );
                continue;
            }
            let hidden_by_parent = file
                .parent()
                .into_iter()
                .flat_map(|parent| parent.ancestors())
                .filter(|path| !path.as_os_str().is_empty())
                .any(|path| {
                    self.skills_view
                        .collapsed_content_directories
                        .contains(path)
                });
            if hidden_by_parent {
                continue;
            }
            let selected = file == self.skills_view.selected_file;
            let file_for_click = file.clone();
            let depth = file.parent().map_or(0, |path| path.components().count()) as f32;
            tree = tree.child(
                div()
                    .id(ElementId::Name(format!("file-{}", file.display()).into()))
                    .h(px(30.))
                    .mx(px(6.))
                    .pl(px(3. + depth * 14.))
                    .pr(px(9.))
                    .rounded(px(RADIUS_LIST_ROW))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .bg(if selected {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        div()
                            .size(px(14.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center(),
                    )
                    .child(
                        div()
                            .size(px(18.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Self::icon("icons/file.svg", 15., p.muted)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .truncate()
                            .child(
                                file.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.skills_view.selected_file = file_for_click.clone();
                        *this.skills_view.content_snapshot.borrow_mut() = None;
                        this.skills_view
                            .content_scroll
                            .set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
            );
        }
        let content = snapshot.content;
        let preview = div()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .px(px(14.))
                    .border_b_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .flex()
                    .items_center()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(p.secondary)
                    .child(self.selectable_text(
                        "content-selected-file",
                        200,
                        self.skills_view.selected_file.display().to_string(),
                        window,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("content-preview-scroll")
                    .track_scroll(&self.skills_view.content_scroll)
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .p(px(22.))
                    .font_family(MONO)
                    .text_size(px(13.))
                    .line_height(relative(1.6))
                    .text_color(p.secondary)
                    .whitespace_normal()
                    .child(self.selectable_text("content-preview", 201, content, window, cx)),
            );
        div().flex_1().min_h_0().flex().child(tree).child(preview)
    }
}

#[cfg(test)]
mod tests {
    use super::installable_skills_by_group;
    use crate::{SkillOrigin, SkillRecord, SkillSummary};
    use std::{collections::HashSet, path::PathBuf};

    fn skill(name: &str, group_id: Option<&str>) -> SkillSummary {
        SkillSummary {
            record: SkillRecord {
                name: name.into(),
                storage_name: format!("storage-{name}"),
                description: String::new(),
                origin: SkillOrigin::Unknown,
                update_available: false,
                group_id: group_id.map(str::to_string),
                last_operated_at: 0,
            },
            path: PathBuf::new(),
            installed_projects: 0,
            manual_only: false,
        }
    }

    #[test]
    fn group_installation_uses_every_library_skill_in_the_group() {
        let skills = vec![
            skill("visible", Some("group-a")),
            skill("filtered-out", Some("group-a")),
            skill("other", Some("group-b")),
        ];
        let groups = HashSet::from(["group-a"]);

        assert_eq!(
            installable_skills_by_group(&skills, &groups)["group-a"],
            ["storage-visible", "storage-filtered-out"]
        );
    }
}
