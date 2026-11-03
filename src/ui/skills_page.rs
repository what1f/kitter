use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SkillListRow {
    Skill(usize, bool),
    Group(usize, bool),
}

fn skill_count(
    p: Palette,
    id: String,
    matched: usize,
    total: usize,
    filtered: bool,
) -> Stateful<Div> {
    let (value, label) = if filtered {
        (
            format!("{matched} / {total}"),
            t!("skills.matching_count", matched = matched, total = total).to_string(),
        )
    } else {
        (
            total.to_string(),
            t!("skills.total_count", count = total).to_string(),
        )
    };
    kit::count(p, &value)
        .font_family(font_ui())
        .font_weight(weight_medium())
        .text_size(px(fs_caption()))
        .h(px(16.))
        .min_w(px(16.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .id(ElementId::Name(id.clone().into()))
        .debug_selector(move || format!("{id}:{value}"))
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
}

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
        selected_keys: Arc<[String]>,
        protected_selection: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let name = skill.record.name.clone();
        let built_in = skill.record.origin.is_builtin();
        let storage_name = skill_storage_name(skill).to_string();
        let context_app = cx.entity().downgrade();
        let selection_count = selected_keys.len();
        let selected = self.skills_view.selection.contains(&storage_name);
        let multi_selection = selected && selection_count > 1;
        let protected_selection = built_in || (multi_selection && protected_selection);
        let install_label = if multi_selection {
            t!("skills.install_selection_count", count = selection_count).to_string()
        } else {
            t!("skills.install_skill").to_string()
        };
        let set_tags_label = if multi_selection {
            t!("skills.set_tags", count = selection_count).to_string()
        } else {
            t!("common.set_tags").to_string()
        };
        let reveal_label = if cfg!(target_os = "macos") {
            t!("common.show_in_finder").to_string()
        } else {
            t!("common.show_in_folder").to_string()
        }
        .to_string();
        let delete_label = if multi_selection {
            t!("skills.delete_skills", count = selection_count).to_string()
        } else {
            t!("skills.delete_skill").to_string()
        };
        let move_group_label = if multi_selection {
            t!("skills.move_skills_to_group", count = selection_count).to_string()
        } else {
            t!("common.move_to_group").to_string()
        };
        let delete_menu_color = p.danger;
        let selection_mode = self.skills_view.selection.is_multiple();
        let context_name = storage_name.clone();
        let click_name = storage_name.clone();
        let click_order = visible_order;
        let reveal_path = skill.path.clone();
        let update_keys = if multi_selection {
            selected_keys.to_vec()
        } else {
            vec![storage_name.clone()]
        };
        let update_enabled = self.model.updating_batch.is_none()
            && self.model.pending_updates.is_none()
            && self.model.skills.iter().any(|skill| {
                update_keys
                    .iter()
                    .any(|key| key == skill_storage_name(skill))
                    && source::can_update(&self.model.library, &skill.record)
            });
        let update_label = t!("skills.update").to_string();
        let checkbox_name = storage_name.clone();
        let trigger_mode = self.model.library.trigger_mode_by_storage(&storage_name);
        let trigger_label = t!("skills.trigger_timing").to_string();
        let preset_label = t!("skills.add_to_preset").to_string();
        let selected_names = if multi_selection {
            selected_keys.to_vec()
        } else {
            Vec::new()
        };
        let preset_choices = self
            .presets_view
            .store
            .presets()
            .iter()
            .map(|preset| {
                let included = if multi_selection {
                    selected_names
                        .iter()
                        .all(|name| preset.skills.contains(name))
                } else {
                    preset.skills.contains(&storage_name)
                };
                (preset.id.clone(), preset.name.clone(), included)
            })
            .collect::<Vec<_>>();
        let trigger_choices = [
            (
                TriggerMode::FollowSkill,
                t!("skills.follow_skill").to_string(),
            ),
            (TriggerMode::Manual, t!("skills.manual_trigger").to_string()),
            (
                TriggerMode::Automatic,
                t!("skills.automatic_trigger").to_string(),
            ),
        ];
        let checkbox = Checkbox::new(ElementId::Name(
            format!("select-skill-{storage_name}").into(),
        ))
        .checked(selected)
        .on_click(cx.listener(move |this, _: &bool, _, cx| {
            cx.stop_propagation();
            this.toggle_skill_selection(checkbox_name.clone(), cx);
        }));
        kit::list_row(
            p,
            ElementId::Name(format!("skill-{storage_name}").into()),
            selected,
        )
        .min_h(px(row_2()))
        .my(px(1.))
        .ml(px(if nested { 12. } else { 0. }))
        .when(selection_mode, |row| row.child(checkbox))
        .child(Self::icon(
            if built_in {
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
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .min_w_0()
                        .child(
                            div()
                                .min_w_0()
                                .font_family(mono())
                                .text_size(px(fs_body()))
                                .font_weight(weight_medium())
                                .truncate()
                                .child(name.clone()),
                        )
                        .when(skill.record.update_available, |line| {
                            line.child(
                                kit::count(p, "●")
                                    .id(ElementId::Name(
                                        format!("update-dot-{storage_name}").into(),
                                    ))
                                    .debug_selector({
                                        let storage = storage_name.clone();
                                        move || format!("update-dot-{storage}")
                                    })
                                    .flex_none()
                                    .text_size(px(8.))
                                    .text_color(p.accent)
                                    .tooltip(|window, cx| {
                                        Tooltip::new(t!("updates.available").to_string())
                                            .build(window, cx)
                                    }),
                            )
                        }),
                )
                .child(
                    div().text_size(px(fs_small())).text_color(p.muted).child(
                        if dirs::home_dir()
                            .is_some_and(|home| self.skill_installed_at(skill, &home, cx))
                        {
                            if skill.installed_projects == 0 {
                                t!("skills.installed_globally").to_string().into()
                            } else {
                                crate::i18n::tn(
                                    "skills.global_projects",
                                    skill.installed_projects,
                                    &[("count", (skill.installed_projects).to_string())],
                                )
                            }
                        } else if skill.installed_projects == 0 {
                            t!("skills.uninstalled").to_string().into()
                        } else {
                            crate::i18n::tn(
                                "skills.project_count",
                                skill.installed_projects,
                                &[("count", (skill.installed_projects).to_string())],
                            )
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
                    skills: if multi_selection {
                        selected_keys.to_vec()
                    } else {
                        vec![storage_name.clone()]
                    },
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
        .context_menu(move |menu, window, menu_cx| {
            let install_app = context_app.clone();
            let update_app = context_app.clone();
            let update_keys = update_keys.clone();
            let set_tags_app = context_app.clone();
            let move_app = context_app.clone();
            let delete_app = context_app.clone();
            let mut menu = menu.min_w(px(210.)).item(
                PopupMenuItem::new(install_label.clone())
                    .icon(Icon::default().path(KitterApp::icon_path("icons/download.svg")))
                    .on_click(move |_, window, cx| {
                        let _ = install_app.update(cx, |this, cx| {
                            this.open_install_dialog(window, cx);
                        });
                    }),
            );
            menu = menu.item(
                PopupMenuItem::new(update_label.clone())
                    .icon(Icon::default().path(KitterApp::icon_path("icons/rotate-cw.svg")))
                    .disabled(!update_enabled)
                    .on_click(move |_, _, cx| {
                        let _ = update_app.update(cx, |this, cx| {
                            this.request_skill_updates(
                                SkillUpdateRequest::Selection(update_keys.clone()),
                                cx,
                            );
                        });
                    }),
            );
            if !multi_selection {
                let reveal_path = reveal_path.clone();
                menu = menu.item(
                    PopupMenuItem::new(reveal_label.clone())
                        .icon(Icon::default().path(KitterApp::icon_path("icons/folder-open.svg")))
                        .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
                );
            }
            menu = menu.item(
                PopupMenuItem::new(set_tags_label.clone())
                    .icon(Icon::default().path(KitterApp::icon_path("icons/hash.svg")))
                    .on_click(move |_, _, cx| {
                        let _ = set_tags_app.update(cx, |this, cx| {
                            let keys = this.selected_skill_keys();
                            this.open_tag_assignment_dialog_for_selection(keys, cx);
                        });
                    }),
            );
            if !multi_selection {
                let trigger_app = context_app.clone();
                let trigger_storage = storage_name.clone();
                let trigger_label = trigger_label.clone();
                let trigger_choices = trigger_choices.clone();
                menu = menu.submenu_with_icon(
                    Some(Icon::default().path(KitterApp::icon_path("icons/clock-3.svg"))),
                    trigger_label,
                    window,
                    menu_cx,
                    move |submenu, _, _| {
                        trigger_choices
                            .iter()
                            .fold(submenu, |submenu, (mode, label)| {
                                let mode = *mode;
                                let app = trigger_app.clone();
                                let storage = trigger_storage.clone();
                                submenu.item(
                                    PopupMenuItem::new(label.clone())
                                        .checked(trigger_mode == mode)
                                        .on_click(move |_, _, cx| {
                                            let _ = app.update(cx, |this, cx| {
                                                this.set_skill_trigger_mode(&storage, mode, cx);
                                            });
                                        }),
                                )
                            })
                    },
                );
            }
            if !preset_choices.is_empty() {
                let preset_app = context_app.clone();
                let preset_choices = preset_choices.clone();
                menu = menu.submenu_with_icon(
                    Some(Icon::default().path(KitterApp::icon_path("icons/layers.svg"))),
                    preset_label.clone(),
                    window,
                    menu_cx,
                    move |submenu, _, _| {
                        preset_choices
                            .iter()
                            .fold(submenu, |submenu, (id, name, included)| {
                                let app = preset_app.clone();
                                let id = id.clone();
                                let included = *included;
                                submenu.item(
                                    PopupMenuItem::new(name.clone()).checked(included).on_click(
                                        move |_, _, cx| {
                                            let _ = app.update(cx, |this, cx| {
                                                let keys = this.selected_skill_keys();
                                                this.toggle_skills_in_preset(
                                                    &id, &keys, included, cx,
                                                );
                                            });
                                        },
                                    ),
                                )
                            })
                    },
                );
            }
            if !protected_selection {
                menu = menu.item(
                    PopupMenuItem::new(move_group_label.clone())
                        .icon(Icon::default().path(KitterApp::icon_path("icons/folder.svg")))
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
        let can_save = self.add_flow.task.is_none()
            && !self
                .groups_flow
                .name_input
                .read(cx)
                .value()
                .trim()
                .is_empty();
        div()
            .id(ElementId::Name(id.to_string().into()))
            .min_h(px(row_1() + 4.))
            .pl(px(if id == "add-group-editor" { 0. } else { 22. }))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                kit::input(p, &self.groups_flow.name_input)
                    .disabled(self.add_flow.task.is_some())
                    .small()
                    .h(px(h_sm()))
                    .min_w_0()
                    .flex_1(),
            )
            .child(
                kit::icon_button(
                    p,
                    "cancel-group-edit",
                    "icons/x.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.groups_flow.edit = None;
                    this.tags_flow.error = None;
                    this.notify_dialog(cx);
                })),
            )
            .child(
                kit::icon_button(
                    p,
                    "save-group-edit",
                    "icons/check.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .when(!can_save, |button| button.opacity(0.4).cursor_default())
                .when(can_save, |button| {
                    button.on_click(cx.listener(|this, _, _, cx| this.commit_group_edit(cx)))
                }),
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
        let group_key = SharedString::from(format!("sortable-group-Management-{}", group.id));
        let mut actions = div()
            .invisible()
            .group_hover(group_key.clone(), |actions| actions.visible())
            .flex()
            .items_center();
        actions = actions.child(
            kit::icon_button(
                p,
                ElementId::Name(format!("delete-group-{}", group_id).into()),
                "icons/trash.svg",
                kit::IconKind::Danger,
                kit::Size::Sm,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_group_delete_dialog(delete_group_id.clone(), cx);
            })),
        );
        kit::list_row(
            p,
            ElementId::Name(format!("group-row-{}", group_id).into()),
            false,
        )
        .group(group_key.clone())
        .h(px(row_1() + 4.))
        .pr(px(4.))
        .cursor_default()
        .when(true, |row| {
            self.sortable_group_row(
                row,
                group_id.clone(),
                group_name.clone(),
                GroupDragScope::Management,
                cx,
            )
        })
        .child(Self::icon("icons/folder.svg", icon_md(), p.secondary))
        .child(
            div()
                .id(ElementId::Name(
                    format!("edit-group-name-{}", group_id).into(),
                ))
                .min_w_0()
                .flex_1()
                .font_family(mono())
                .text_size(px(fs_body()))
                .truncate()
                .cursor_pointer()
                .child(group_name)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_group_edit(GroupEdit::Rename(rename_group_id.clone()), window, cx);
                })),
        )
        .child(actions)
        .into_any_element()
    }

    pub(super) fn group_delete_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
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
        let close = kit::close_button(p, "close-delete-group", true)
            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)));
        let cancel = kit::button(
            p,
            "cancel-delete-group",
            kit::ButtonKind::Ghost,
            kit::Size::Md,
            true,
        )
        .child(t!("common.cancel").to_string())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| this.close_dialog(cx)),
        );
        let confirm = kit::button(
            p,
            "confirm-delete-group",
            kit::ButtonKind::Danger,
            kit::Size::Md,
            true,
        )
        .child(if delete_skills {
            t!("skills.delete_group_and_skills").to_string()
        } else {
            t!("skills.delete_group").to_string()
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.delete_skill_group(group.id.clone(), delete_skills, cx);
            }),
        );
        kit::modal_panel(p)
            .child(kit::modal_header(
                t!("skills.delete_group").to_string(),
                close,
            ))
            .child(
                kit::modal_body()
                    .pt(px(4.))
                    .text_size(px(fs_body()))
                    .line_height(relative(1.5))
                    .child(t!("skills.delete_group_confirm", name = group.name).to_string())
                    .child(
                        div()
                            .mt(px(4.))
                            .text_color(p.secondary)
                            .child(t!("skills.delete_group_hint").to_string()),
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
                                .child(div().child(crate::i18n::tn(
                                    "skills.delete_group_skills",
                                    count,
                                    &[("count", (count).to_string())],
                                )))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.groups_flow.delete_skills =
                                        !this.groups_flow.delete_skills;
                                    this.notify_dialog(cx);
                                })),
                        )
                    })
                    .when(delete_skills, |body| {
                        body.child(
                            kit::field_error(p, t!("skills.delete_warning").to_string()).mt(px(8.)),
                        )
                    })
                    .when_some(self.tags_flow.error.clone(), |body, error| {
                        body.child(kit::field_error(p, error).mt(px(8.)))
                    }),
            )
            .child(kit::modal_footer(p).child(cancel).child(confirm))
    }

    pub(super) fn group_management_modal(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let groups = self.model.library.groups();
        let mut rows = div().flex().flex_col().gap(px(2.));
        if self.groups_flow.edit == Some(GroupEdit::Create) {
            rows = rows.child(self.group_inline_editor("new-group-row", cx));
        }
        if groups.is_empty() && self.groups_flow.edit != Some(GroupEdit::Create) {
            rows = rows.child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(fs_body()))
                    .text_color(p.muted)
                    .child(t!("skills.no_groups_yet").to_string()),
            );
        }
        for group in groups {
            rows = rows.child(self.group_management_row(group, cx));
        }
        rows = rows.child(
            kit::button(
                p,
                "new-group-at-end",
                kit::ButtonKind::Quiet,
                kit::Size::Sm,
                true,
            )
            .mt(px(6.))
            .self_start()
            .child(Self::icon("icons/plus.svg", icon_sm(), p.secondary))
            .child(t!("organize.new_group").to_string())
            .on_click(cx.listener(|this, _, window, cx| {
                this.start_group_edit(GroupEdit::Create, window, cx);
            })),
        );
        let close = kit::close_button(p, "close-groups-modal", true)
            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)));
        kit::modal_panel(p)
            .child(kit::modal_header(
                t!("common.manage_groups").to_string(),
                close,
            ))
            .child(
                div()
                    .id("group-manager-scroll")
                    .pl(px(32.))
                    .pr(px(20.))
                    .pt(px(4.))
                    .pb(px(20.))
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
                    .child(rows)
                    .when_some(self.tags_flow.error.clone(), |panel, error| {
                        panel.child(kit::field_error(p, error).mt(px(8.)))
                    }),
            )
    }

    /// Row above a list showing the active tag filter. Hidden until the
    /// scope has tags, since the menu would only offer "All".
    pub(super) fn tag_filter_bar(&self, scope: TagScope, cx: &mut Context<Self>) -> Option<Div> {
        let tags = self.tags_for(scope);
        if tags.tags().is_empty() {
            return None;
        }
        let p = self.palette();
        let label = self
            .tag_filter_for(scope)
            .and_then(|tag| tags.path(tag))
            .map(|path| format!("#{path}"))
            .unwrap_or_else(|| match scope {
                TagScope::Skills => t!("common.all_skills").to_string(),
            });
        Some(
            div()
                .pl(px(16.))
                .pr(px(12.))
                .pb(px(4.))
                .flex()
                .items_center()
                .text_size(px(fs_small()))
                .text_color(p.muted)
                .child(div().min_w_0().flex_1().truncate().child(label))
                .child(self.tag_filter_control(scope, cx)),
        )
    }

    pub(super) fn tag_filter_control(&self, scope: TagScope, cx: &mut Context<Self>) -> Popover {
        let p = self.palette();
        let selected = self.tag_filter_for(scope);
        let tag_state = self.tags_for(scope);
        let tags = tag_state
            .tags()
            .iter()
            .map(|tag| (tag.id, tag.name.clone(), tag.parent))
            .collect::<Vec<_>>();
        let scope_prefix = match scope {
            TagScope::Skills => "skill",
        };
        let all_label = match scope {
            TagScope::Skills => t!("common.all_skills").to_string(),
        };
        let app = cx.entity().downgrade();
        Popover::new(ElementId::Name(
            format!("{scope_prefix}-tag-filter-menu").into(),
        ))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            kit::icon_trigger(
                p,
                ElementId::Name(format!("{scope_prefix}-tag-filter-trigger").into()),
                "icons/hash.svg",
                t!("common.tags").to_string(),
            )
            .selected_fill(p, selected.is_some()),
        )
        .content(move |_, _, popover_cx| {
            let mut menu = kit::menu(p).w(px(220.));
            let all_app = app.clone();
            menu = menu.child(
                kit::menu_item(
                    p,
                    ElementId::Name(format!("{scope_prefix}-tag-filter-all").into()),
                    all_label.clone(),
                    selected.is_none(),
                )
                .on_click(popover_cx.listener(move |_, _, _, cx| {
                    let _ = all_app.update(cx, |this, cx| {
                        this.set_tag_filter(scope, None);
                        cx.notify();
                    });
                    cx.emit(DismissEvent);
                })),
            );
            for (id, name, _parent) in tags.iter().filter(|tag| tag.2.is_none()) {
                let root_app = app.clone();
                let root_id = *id;
                menu = menu.child(
                    kit::menu_item(
                        p,
                        ElementId::Name(format!("{scope_prefix}-tag-filter-{id}").into()),
                        format!("#{name}"),
                        selected == Some(*id),
                    )
                    .on_click(popover_cx.listener(move |_, _, _, cx| {
                        let _ = root_app.update(cx, |this, cx| {
                            this.set_tag_filter(scope, Some(root_id));
                            cx.notify();
                        });
                        cx.emit(DismissEvent);
                    })),
                );
                for (child_id, child_name, _) in tags.iter().filter(|tag| tag.2 == Some(*id)) {
                    let child_app = app.clone();
                    let child_id = *child_id;
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("{scope_prefix}-tag-filter-{child_id}").into()),
                            format!("#{child_name}"),
                            selected == Some(child_id),
                        )
                        .pl(px(24.))
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = child_app.update(cx, |this, cx| {
                                    this.set_tag_filter(scope, Some(child_id));
                                    cx.notify();
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
            }
            menu
        })
    }

    fn skill_group_row(
        &self,
        group: &SkillGroup,
        collapsed: bool,
        install_group_skills: Vec<String>,
        matched_count: usize,
        filtered: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let group_id = group.id.clone();
        let group_label = group.name.clone();
        let toggle_key = group_id.clone();
        let drop_group_id = group_id.clone();
        let context_app = cx.entity().downgrade();
        let rename_group_label = t!("common.rename").to_string();
        let delete_group_label = t!("skills.delete_group").to_string();
        let rename_group_id = group_id.clone();
        let delete_group_id = group_id.clone();
        let delete_group_menu_color = p.danger;
        let install_group_count = install_group_skills.len();
        let update_group_skills = self
            .model
            .skills
            .iter()
            .filter(|skill| {
                skill.record.group_id.as_deref() == Some(group.id.as_str())
                    && source::can_update(&self.model.library, &skill.record)
            })
            .map(|skill| skill_storage_name(skill).to_string())
            .collect::<Vec<_>>();
        let update_enabled = !update_group_skills.is_empty()
            && self.model.updating_batch.is_none()
            && self.model.pending_updates.is_none();
        let update_group_label = t!("skills.update").to_string();
        let preset_label = t!("skills.add_to_preset").to_string();
        let preset_choices = self
            .presets_view
            .store
            .presets()
            .iter()
            .map(|preset| {
                let included = install_group_skills
                    .iter()
                    .all(|name| preset.skills.contains(name));
                (preset.id.clone(), preset.name.clone(), included)
            })
            .collect::<Vec<_>>();
        let install_group_label = crate::i18n::tn(
            "skills.install_group_count",
            install_group_count,
            &[("count", (install_group_count).to_string())],
        );
        let row = kit::list_row(
            p,
            ElementId::Name(format!("skill-group-{group_id}").into()),
            false,
        )
        .h(px(row_1()))
        .mt(px(4.))
        .text_color(p.secondary)
        .when(true, |row| {
            self.sortable_group_row(
                row,
                group_id.clone(),
                group_label.clone(),
                GroupDragScope::List,
                cx,
            )
        })
        .child(Self::icon(
            if collapsed {
                "icons/chevron-right.svg"
            } else {
                "icons/chevron-down.svg"
            },
            icon_sm(),
            p.muted,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(7.))
                .child(
                    div()
                        .min_w_0()
                        .font_family(mono())
                        .text_size(px(fs_body()))
                        .font_weight(weight_medium())
                        .truncate()
                        .child(group_label),
                )
                .when(
                    collapsed
                        && self.model.skills.iter().any(|skill| {
                            skill.record.group_id.as_deref() == Some(group_id.as_str())
                                && skill.record.update_available
                        }),
                    |row| {
                        row.child(
                            kit::count(p, "●")
                                .id(ElementId::Name(
                                    format!("update-group-dot-{group_id}").into(),
                                ))
                                .debug_selector({
                                    let group = group_id.clone();
                                    move || format!("update-group-dot-{group}")
                                })
                                .flex_none()
                                .text_size(px(8.))
                                .text_color(p.accent)
                                .tooltip(|window, cx| {
                                    Tooltip::new(t!("updates.available").to_string())
                                        .build(window, cx)
                                }),
                        )
                    },
                ),
        )
        .child(skill_count(
            p,
            format!("skill-group-count-{}", group.id),
            matched_count,
            install_group_count,
            filtered,
        ))
        .drag_over::<SkillDrag>(move |style, _, _, _| style.drop_fill(p))
        .on_drop(cx.listener(move |this, drag: &SkillDrag, _, cx| {
            this.assign_skills_group(drag.skills.clone(), Some(drop_group_id.clone()), cx);
        }))
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.skills_view.collapsed_groups.remove(&toggle_key) {
                this.skills_view.collapsed_groups.insert(toggle_key.clone());
            }
            this.persist_collapsed_groups();
            cx.notify();
        }))
        .context_menu(move |menu, window, menu_cx| {
            let install_app = context_app.clone();
            let update_app = context_app.clone();
            let update_group_skills = update_group_skills.clone();
            let preset_app = context_app.clone();
            let preset_skills = install_group_skills.clone();
            let preset_choices = preset_choices.clone();
            let rename_app = context_app.clone();
            let delete_app = context_app.clone();
            let install_group_skills = install_group_skills.clone();
            let rename_group_id = rename_group_id.clone();
            let delete_group_id = delete_group_id.clone();
            let delete_menu_item =
                Self::danger_menu_item(delete_group_label.clone(), delete_group_menu_color)
                    .on_click(move |_, _, cx| {
                        let _ = delete_app.update(cx, |this, cx| {
                            this.open_group_delete_dialog(delete_group_id.clone(), cx);
                        });
                    });
            menu.min_w(px(190.))
                .when(!install_group_skills.is_empty(), |menu| {
                    menu.item(
                        PopupMenuItem::new(install_group_label.clone())
                            .icon(Icon::default().path(KitterApp::icon_path("icons/download.svg")))
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
                    PopupMenuItem::new(update_group_label.clone())
                        .icon(Icon::default().path(KitterApp::icon_path("icons/rotate-cw.svg")))
                        .disabled(!update_enabled)
                        .on_click(move |_, _, cx| {
                            let _ = update_app.update(cx, |this, cx| {
                                this.request_skill_updates(
                                    SkillUpdateRequest::Selection(update_group_skills.clone()),
                                    cx,
                                );
                            });
                        }),
                )
                .when(
                    !preset_skills.is_empty() && !preset_choices.is_empty(),
                    |menu| {
                        menu.submenu_with_icon(
                            Some(Icon::default().path(KitterApp::icon_path("icons/layers.svg"))),
                            preset_label.clone(),
                            window,
                            menu_cx,
                            move |submenu, _, _| {
                                preset_choices.iter().fold(
                                    submenu,
                                    |submenu, (id, name, included)| {
                                        let app = preset_app.clone();
                                        let id = id.clone();
                                        let skills = preset_skills.clone();
                                        let included = *included;
                                        submenu.item(
                                            PopupMenuItem::new(name.clone())
                                                .checked(included)
                                                .on_click(move |_, _, cx| {
                                                    let _ = app.update(cx, |this, cx| {
                                                        this.toggle_skills_in_preset(
                                                            &id, &skills, included, cx,
                                                        );
                                                    });
                                                }),
                                        )
                                    },
                                )
                            },
                        )
                    },
                )
                .item(
                    PopupMenuItem::new(rename_group_label.clone())
                        .icon(Icon::default().path(KitterApp::icon_path("icons/pencil.svg")))
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
        });
        row.into_any_element()
    }

    fn start_description_edit(
        &mut self,
        storage_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(description) = self
            .model
            .skills
            .iter()
            .find(|skill| skill_storage_name(skill) == storage_name)
            .map(|skill| skill.record.description.clone())
        else {
            return;
        };
        self.skills_view.description_input.update(cx, |input, cx| {
            input.set_value(description, window, cx);
            input.focus_handle(cx).focus(window, cx);
        });
        self.skills_view.editing_description = Some(storage_name);
        cx.notify();
    }

    pub(super) fn commit_description_edit(&mut self, cx: &mut Context<Self>) {
        let Some(storage_name) = self.skills_view.editing_description.take() else {
            return;
        };
        let value = self
            .skills_view
            .description_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let unchanged = self
            .model
            .skills
            .iter()
            .find(|skill| skill_storage_name(skill) == storage_name)
            .is_none_or(|skill| skill.record.description == value);
        if !unchanged {
            // Matching the SKILL.md text again means going back to it.
            let original = self
                .model
                .skills
                .iter()
                .find(|skill| skill_storage_name(skill) == storage_name)
                .is_some_and(|skill| skill.original_description == value);
            let value = if original { "" } else { value.as_str() };
            match self
                .model
                .library
                .set_description_by_storage(&storage_name, value)
            {
                Ok(()) => self.refresh(cx),
                Err(error) => {
                    let message = self.error_message(error);
                    self.show_notice(message, cx);
                }
            }
        }
        cx.notify();
    }

    fn sync_skill_list_rows(&self, rows: Vec<SkillListRow>) -> Arc<[SkillListRow]> {
        let mut previous = self.skills_view.list_rows.borrow_mut();
        if previous.as_ref() != rows.as_slice() {
            let prefix = previous
                .iter()
                .zip(&rows)
                .take_while(|(a, b)| a == b)
                .count();
            let suffix = previous[prefix..]
                .iter()
                .rev()
                .zip(rows[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            self.skills_view.list.splice(
                prefix..previous.len() - suffix,
                rows.len() - prefix - suffix,
            );
            *previous = rows.into();
        }
        Arc::clone(&previous)
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
                let matches_query = self.skill_matches_query(skill, &query);
                let matches_tag = self.tags_flow.selected_skill_filter.is_none_or(|tag| {
                    self.tags_flow
                        .skills
                        .matches_filter(skill_storage_name(skill), tag)
                });
                matches_query && matches_tag
            })
            .collect::<Vec<_>>();
        let visible_count = visible_skills.len();
        let filtered = !query.is_empty() || self.tags_flow.selected_skill_filter.is_some();
        let configured_groups = self.model.library.groups();
        let group_ids = configured_groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<HashSet<_>>();
        let installable_group_skills = installable_skills_by_group(&self.model.skills, &group_ids);
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
            let collapsed = !filtered && self.skills_view.collapsed_groups.contains(&group.id);
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
        let mut list = kit::pane(p)
            .w_full()
            .child(
                self.panel_header_content(
                    "skills-panel-window-drag",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.))
                        .child(kit::pane_title(t!("common.skills").to_string()))
                        .child(skill_count(
                            p,
                            "skills-total-count".into(),
                            visible_count,
                            self.model.skills.len(),
                            filtered,
                        )),
                    cx,
                )
                .child(self.skills_manage_control(cx))
                .child(div().w(px(8.)).flex_none())
                .child(
                    self.labeled_icon_button(
                        "add-skill",
                        "icons/plus.svg",
                        t!("common.add_skill").to_string(),
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
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .child(kit::search_input(p, &self.skills_view.skill_search).small()),
            )
            .children(self.tag_filter_bar(TagScope::Skills, cx))
            .when(self.skills_view.selection.is_multiple(), |list| {
                let visible_selection_order = visible_selection_order.clone();
                list.child(
                    div()
                        .pl(px(16.))
                        .pr(px(12.))
                        .pb(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .text_size(px(fs_small()))
                        .text_color(p.secondary)
                        .child(crate::i18n::tn(
                            "skills.selection_count",
                            selection_count,
                            &[("count", (selection_count).to_string())],
                        ))
                        .child(div().flex_1())
                        .child(
                            kit::button(
                                p,
                                "select-all-visible-skills",
                                kit::ButtonKind::Quiet,
                                kit::Size::Sm,
                                true,
                            )
                            .h(px(24.))
                            .px(px(6.))
                            .text_size(px(fs_small()))
                            .child(t!("skills.select_all_short").to_string())
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.select_all_visible_skills(
                                        visible_selection_order.as_ref(),
                                        cx,
                                    );
                                },
                            )),
                        )
                        .child(
                            kit::button(
                                p,
                                "clear-skill-selection",
                                kit::ButtonKind::Quiet,
                                kit::Size::Sm,
                                true,
                            )
                            .h(px(24.))
                            .px(px(6.))
                            .text_size(px(fs_small()))
                            .child(t!("skills.clear").to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_skill_selection(cx);
                            })),
                        ),
                )
            });
        let skill_indices = self
            .model
            .skills
            .iter()
            .enumerate()
            .map(|(index, skill)| (skill_storage_name(skill), index))
            .collect::<std::collections::HashMap<_, _>>();
        let mut flat_rows = Vec::new();
        flat_rows.extend(
            pinned
                .iter()
                .map(|skill| SkillListRow::Skill(skill_indices[skill_storage_name(skill)], false)),
        );
        for (index, group) in configured_groups.iter().enumerate() {
            if filtered && grouped.get(&group.id).is_none_or(Vec::is_empty) {
                continue;
            }
            let collapsed = !filtered && self.skills_view.collapsed_groups.contains(&group.id);
            flat_rows.push(SkillListRow::Group(index, collapsed));
            if !collapsed && let Some(skills) = grouped.get(&group.id) {
                flat_rows.extend(skills.iter().map(|skill| {
                    SkillListRow::Skill(skill_indices[skill_storage_name(skill)], true)
                }));
            }
        }
        flat_rows.extend(
            ungrouped
                .iter()
                .map(|skill| SkillListRow::Skill(skill_indices[skill_storage_name(skill)], false)),
        );
        let rows = self.sync_skill_list_rows(flat_rows);
        let matched_group_counts = grouped
            .iter()
            .map(|(id, skills)| (id.clone(), skills.len()))
            .collect::<BTreeMap<_, _>>();
        let groups: Arc<[SkillGroup]> = configured_groups.into();
        let selected_keys: Arc<[String]> = self.selected_skill_keys().into();
        let protected_selection = self.model.skills.iter().any(|skill| {
            skill.record.origin.is_builtin()
                && self
                    .skills_view
                    .selection
                    .contains(skill_storage_name(skill))
        });
        let virtual_list = gpui::list(
            self.skills_view.list.clone(),
            cx.processor(move |this, index, _, cx| {
                let row = match rows[index] {
                    SkillListRow::Skill(index, nested) => this.skill_list_row(
                        &this.model.skills[index],
                        nested,
                        Arc::clone(&selection_order),
                        Arc::clone(&selected_keys),
                        protected_selection,
                        cx,
                    ),
                    SkillListRow::Group(index, collapsed) => {
                        let group = &groups[index];
                        this.skill_group_row(
                            group,
                            collapsed,
                            installable_group_skills
                                .get(&group.id)
                                .cloned()
                                .unwrap_or_default(),
                            matched_group_counts.get(&group.id).copied().unwrap_or(0),
                            filtered,
                            cx,
                        )
                    }
                };
                div().w_full().child(row).into_any_element()
            }),
        )
        .w_full()
        .h_full();
        list = list.child(
            div()
                .id("skill-groups-scroll")
                .debug_selector(|| "skill-groups-scroll".into())
                .flex_1()
                .min_h_0()
                .px(px(8.))
                .pb(px(8.))
                .relative()
                .overflow_hidden()
                .child(virtual_list)
                .child(kit::scrollbar(
                    "skill-list-scrollbar",
                    &self.skills_view.list,
                )),
        );
        layout::content(
            &self.shell.content_layout,
            self.content_pane_width(window, cx),
            list,
            self.skill_detail(window, cx),
        )
    }

    pub(super) fn skill_tag_controls(&self, skill_name: &str, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
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
            .gap(px(6.))
            .flex_wrap();
        for (tag_id, path) in assigned {
            labels = labels.child(
                kit::chip(p)
                    .id(ElementId::Name(format!("skill-tag-{tag_id}").into()))
                    .cursor_pointer()
                    .font_family(mono())
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
            gpui_base::Button::new(ElementId::Name(
                format!("skill-tags-trigger-{skill_name}").into(),
            ))
            .h(px(24.))
            .px(px(8.))
            .rounded(px(r_sm()))
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(px(fs_small()))
            .text_color(p.secondary)
            .hover(move |button| button.bg(p.hover).text_color(p.text))
            .styles(|styles| styles.selected(|style| style.selected_fill(p, true)))
            .when(!has_assigned, |button| {
                button.child(Self::icon("icons/plus.svg", icon_xs(), p.secondary))
            })
            .child(if has_assigned {
                t!("skills.edit").to_string()
            } else {
                t!("skills.add_tag").to_string()
            }),
        )
        .content(move |_, _, popover_cx| {
            let mut menu = kit::menu(p).w(px(240.));
            if tag_rows.is_empty() {
                menu = menu.child(
                    div()
                        .px(px(8.))
                        .py(px(8.))
                        .text_size(px(fs_body()))
                        .text_color(p.muted)
                        .child(t!("common.no_tags_yet").to_string()),
                );
            }
            for (id, name, parent, checked) in &tag_rows {
                let tag_app = app.clone();
                let tag_skill = skill.clone();
                let tag_id = *id;
                menu = menu.child(
                    kit::menu_item(
                        p,
                        ElementId::Name(format!("assign-tag-{tag_id}").into()),
                        format!("#{}", name),
                        *checked,
                    )
                    .when(parent.is_some(), |row| row.pl(px(24.)))
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
            menu.child(kit::menu_separator(p)).child(
                kit::menu_action(
                    p,
                    "new-tag-from-skill",
                    "icons/plus.svg",
                    t!("organize.new_tag").to_string(),
                )
                .on_click(popover_cx.listener(move |_, _, _, cx| {
                    let _ = new_tag_app
                        .update(cx, |this, cx| this.open_tag_dialog(TagScope::Skills, cx));
                    cx.emit(DismissEvent);
                })),
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
                kit::inset_box(p)
                    .h(px(row_1()))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Self::icon("icons/package.svg", icon_sm(), p.secondary))
                    .child(
                        div()
                            .min_w_0()
                            .font_family(mono())
                            .text_size(px(fs_body()))
                            .truncate()
                            .child(name.clone()),
                    ),
            );
        }
        if selection_count > preview_count {
            names = names.child(
                div()
                    .mt(px(4.))
                    .text_size(px(fs_small()))
                    .text_color(p.muted)
                    .child(
                        t!("skills.and_more", count = selection_count - preview_count).to_string(),
                    ),
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
            .pt(px(24.))
            .child(kit::detail_header(
                p,
                "icons/package.svg",
                crate::i18n::tn(
                    "skills.selection_count",
                    selection_count,
                    &[("count", (selection_count).to_string())],
                ),
                t!("skills.selection_hint").to_string(),
                self.labeled_icon_button(
                    "install-selected-skills",
                    "icons/download.svg",
                    t!("skills.install_selection").to_string(),
                    cx,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.open_install_dialog(window, cx);
                    }),
                ),
            ))
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
                .child(t!("skills.empty_hint").to_string())
                .child(self.window_drag_strip("empty-skill-detail-window-drag", 24., cx));
        };
        let storage_name = skill_storage_name(skill).to_string();
        let skill_path = skill.path.clone();
        let mut actions = div().flex().items_center().gap(px(4.));
        let updating = self.model.updating_skill.as_deref() == Some(storage_name.as_str());
        if skill.record.update_available || updating {
            let update_name = storage_name.clone();
            actions = actions.child(
                kit::button(
                    p,
                    "update-skill",
                    kit::ButtonKind::Ghost,
                    kit::Size::Sm,
                    updating
                        || (self.model.updating_batch.is_none()
                            && self.model.updating_skill.is_none()
                            && self.model.pending_updates.is_none()),
                )
                .when(updating, |button| {
                    button.cursor_default().child(
                        div()
                            .id("update-skill-progress")
                            .debug_selector(|| "update-skill-progress".to_string())
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(icon_sm()))
                            .child(self.shell.spinner_update.clone()),
                    )
                })
                .when(!updating, |button| {
                    button.child(
                        div()
                            .id("update-skill-refresh")
                            .debug_selector(|| "update-skill-refresh".to_string())
                            .flex()
                            .child(Self::icon("icons/rotate-cw.svg", icon_sm(), p.secondary)),
                    )
                })
                .child(t!("skills.update").to_string())
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
                self.labeled_icon_button(
                    "install-skill",
                    "icons/download.svg",
                    t!("skills.install_skill").to_string(),
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
            t!("skills.description_empty").to_string()
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
                    font: font(font_ui()),
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
            .font_family(font_ui())
            .text_size(px(fs_emph()))
            .line_height(relative(1.5))
            .text_color(p.secondary)
            .child(description);
        if can_expand_description && !description_expanded {
            description_text = description_text
                .line_clamp(DESCRIPTION_COLLAPSED_LINES)
                .text_ellipsis()
                .child(kit::clamp_fade(p, "skill-description"));
        }
        let editing_description =
            self.skills_view.editing_description.as_deref() == Some(storage_name.as_str());
        let edit_storage_name = storage_name.clone();
        let mut description_section = div()
            .mt(px(16.))
            .max_w(px(DESCRIPTION_MAX_WIDTH))
            .when(editing_description, |section| {
                section.child(
                    div()
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
                                this.skills_view.editing_description = None;
                                cx.notify();
                            }
                        }))
                        .child(kit::textarea(p, &self.skills_view.description_input)),
                )
            })
            .when(!editing_description, |section| {
                section.child(
                    div()
                        .id("skill-description")
                        .group("skill-description")
                        .mx(px(-6.))
                        .px(px(6.))
                        .rounded(px(r_sm()))
                        .cursor_text()
                        .hover_fill(p)
                        .child(description_text)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.start_description_edit(edit_storage_name.clone(), window, cx);
                        })),
                )
            });
        if can_expand_description && !editing_description {
            let description_name = skill.record.name.clone();
            let toggle_label = if description_expanded {
                t!("skills.less").to_string()
            } else {
                t!("skills.more").to_string()
            };
            description_section = description_section.child(
                div().w_full().mt(px(4.)).flex().items_center().child(
                    kit::button(
                        p,
                        "toggle-skill-description",
                        kit::ButtonKind::Quiet,
                        kit::Size::Sm,
                        true,
                    )
                    .h(px(24.))
                    .px(px(6.))
                    .ml(px(-6.))
                    .gap(px(4.))
                    .text_size(px(fs_small()))
                    .child(toggle_label)
                    .child(Self::icon(
                        if description_expanded {
                            "icons/chevron-up.svg"
                        } else {
                            "icons/chevron-down.svg"
                        },
                        icon_xs(),
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
            .child(kit::detail_header(
                p,
                "icons/package.svg",
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .font_family(mono())
                    .child(self.selectable_text(
                        "skill-detail-name",
                        100,
                        skill.record.name.clone(),
                        window,
                        cx,
                    ))
                    .when_some(skill.version.clone(), |title, version| {
                        title.child(
                            kit::count(p, version)
                                .id("skill-detail-version")
                                .debug_selector(|| "skill-detail-version".into())
                                .max_w(px(140.))
                                .truncate(),
                        )
                    })
                    .when(skill.manual_only, |title| {
                        title.child(self.manual_skill_badge())
                    }),
                self.selectable_text(
                    "skill-detail-origin",
                    101,
                    self.source_label(&skill.record.origin),
                    window,
                    cx,
                ),
                actions,
            ))
            .child(description_section)
            .child(self.skill_tag_controls(&storage_name, cx));
        let tabs = kit::tab_bar(p)
            .child(self.tab_button(
                "tab-installs",
                t!("skills.installs").to_string(),
                DetailTab::Installs,
                cx,
            ))
            .child(self.tab_button(
                "tab-content",
                t!("skills.content").to_string(),
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
        label: impl Into<SharedString>,
        tab: DetailTab,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label: SharedString = label.into();
        let active = self.skills_view.tab == tab;
        kit::tab(self.palette(), id, active)
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
        self.project_snapshot(root, cx)
            .map(|snapshot| snapshot.installations_for(skill).to_vec())
            .unwrap_or_default()
    }

    pub(super) fn skill_installed_at(
        &self,
        skill: &SkillSummary,
        root: &PathBuf,
        cx: &mut Context<Self>,
    ) -> bool {
        self.project_snapshot(root, cx)
            .is_some_and(|snapshot| !snapshot.installations_for(skill).is_empty())
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
                kit::empty_state(
                    p,
                    "icons/package.svg",
                    t!("skills.installation_empty").to_string(),
                )
                .mt(px(32.)),
            );
        } else {
            let mut card = kit::card(p);
            for (index, path) in projects.into_iter().enumerate() {
                let popover_id = format!("agents:{}:{}", skill.record.name, path.display());
                let project_name = if home.as_ref() == Some(&path) {
                    t!("common.global").to_string()
                } else {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                };
                let project_path = display_path(&path);
                let installations = self.skill_installations_at(skill, &path, cx);
                let agents = self
                    .context_estimate_snapshot(&path, cx)
                    .map(|snapshot| snapshot.agents_for_skill(skill).to_vec())
                    .unwrap_or_default();
                let project_for_delete = path.clone();
                let skill_for_delete = ProjectSkill {
                    name: skill.record.name.clone(),
                    installations,
                };
                card = card.child(
                    kit::card_row(p, index == 0)
                        .h(px(56.))
                        .child(Self::icon("icons/folder.svg", icon_md(), p.secondary))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(div().font_family(mono()).text_size(px(fs_body())).child(
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
                                        .mt(px(2.))
                                        .font_family(mono())
                                        .text_size(px(fs_small()))
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
                        .child(self.effective_agent_badges(popover_id, &agents, cx))
                        .child(
                            self.danger_icon_button(
                                ElementId::Name(format!("remove-install-project-{index}").into()),
                                "icons/trash.svg",
                                cx,
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.delete_flow.selected =
                                        unique_installation_paths(&skill_for_delete.installations)
                                            .into_iter()
                                            .collect();
                                    this.delete_flow.confirmation =
                                        Some(DeleteConfirmation::ProjectSkills {
                                            project: project_for_delete.clone(),
                                            skills: vec![skill_for_delete.clone()],
                                            batch: false,
                                        });
                                    this.open_delete_dialog(window, cx);
                                }),
                            ),
                        ),
                );
            }
            body = body.child(card);
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
        let mut tree = kit::pane(p)
            .id("content-tree-scroll")
            .w(px(220.))
            .py(px(8.))
            .track_scroll(&self.skills_view.content_tree_scroll)
            .overflow_y_scroll();
        for entry in file_tree::rows(
            p,
            "",
            files,
            None,
            &self.skills_view.selected_file,
            &self.skills_view.collapsed_content_directories,
        ) {
            let path = entry.path;
            tree = tree.child(if entry.directory {
                entry.row.on_click(cx.listener(move |this, _, _, cx| {
                    let collapsed = &mut this.skills_view.collapsed_content_directories;
                    if !collapsed.remove(&path) {
                        collapsed.insert(path.clone());
                    }
                    cx.notify();
                }))
            } else {
                entry.row.on_click(cx.listener(move |this, _, _, cx| {
                    this.skills_view.selected_file = path.clone();
                    *this.skills_view.content_snapshot.borrow_mut() = None;
                    this.skills_view
                        .content_scroll
                        .set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }))
            });
        }
        let content = snapshot.content;
        let preview = div()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(kit::file_bar(p).child(self.selectable_text(
                "content-selected-file",
                200,
                self.skills_view.selected_file.display().to_string(),
                window,
                cx,
            )))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("content-preview-scroll")
                            .track_scroll(&self.skills_view.content_scroll)
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .p(px(24.))
                            .font_family(mono())
                            .text_size(px(fs_body()))
                            .line_height(relative(1.6))
                            .text_color(p.secondary)
                            .whitespace_normal()
                            .child(self.selectable_text(
                                "content-preview",
                                201,
                                content,
                                window,
                                cx,
                            )),
                    )
                    .child(kit::scrollbar(
                        "content-preview-scrollbar",
                        &self.skills_view.content_scroll,
                    )),
            );
        let tree = div()
            .relative()
            .h_full()
            .flex_none()
            .child(tree)
            .child(kit::scrollbar(
                "content-tree-scrollbar",
                &self.skills_view.content_tree_scroll,
            ));
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
            original_description: String::new(),
            version: None,
            record: SkillRecord {
                name: name.into(),
                storage_name: format!("storage-{name}"),
                description: String::new(),
                custom_description: None,
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
