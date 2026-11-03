use super::*;

impl KitterApp {
    pub(super) fn sortable_group_row(
        &self,
        row: Stateful<Div>,
        id: String,
        name: String,
        scope: GroupDragScope,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let key: SharedString = format!("sortable-group-{scope:?}-{id}").into();
        let target_id = id.clone();
        let drop_id = id.clone();
        let position = self
            .groups_flow
            .drop_target
            .as_ref()
            .filter(|(origin, target, _)| *origin == scope && target == &id && cx.has_active_drag())
            .map(|(_, _, position)| *position);
        let row_selector = format!("sort-group-{id}");
        let drag = GroupDrag {
            scope,
            id: id.clone(),
            name,
        };
        let row = if scope == GroupDragScope::Management {
            let handle = div()
                .id(ElementId::Name(format!("drag-group-{id}").into()))
                .debug_selector(|| format!("drag-group-{id}"))
                .absolute()
                .left_0()
                .top(px(5.))
                .w(px(24.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .cursor(CursorStyle::OpenHand)
                .opacity(0.)
                .tab_index(0)
                .focus_visible(|handle| handle.opacity(1.))
                .group_hover(key.clone(), |handle| handle.opacity(1.))
                .child(kit::grip(p))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()));
            row.ml(px(-24.)).pl(px(24.)).child(handle)
        } else {
            row.on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
        };
        row.debug_selector(|| row_selector)
            .group(key)
            .relative()
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<GroupDrag>, _, cx| {
                    if event.drag(cx).scope != scope {
                        return;
                    }
                    let position = (event.bounds.contains(&event.event.position)
                        && event.drag(cx).id != target_id)
                        .then(|| {
                            if event.event.position.y < event.bounds.center().y {
                                TagDropPosition::Before
                            } else {
                                TagDropPosition::After
                            }
                        });
                    let next = position.map(|position| (scope, target_id.clone(), position));
                    if next.is_some()
                        || this
                            .groups_flow
                            .drop_target
                            .as_ref()
                            .is_some_and(|(origin, id, _)| *origin == scope && id == &target_id)
                    {
                        if this.groups_flow.drop_target != next {
                            this.groups_flow.drop_target = next;
                            this.notify_dialog(cx);
                            cx.notify();
                        }
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &GroupDrag, _, cx| {
                if drag.scope != scope {
                    return;
                }
                if let Some((_, id, position)) = this
                    .groups_flow
                    .drop_target
                    .take()
                    .filter(|(origin, id, _)| *origin == scope && id == &drop_id)
                {
                    if let Err(error) = this.model.library.move_group(
                        &drag.id,
                        &id,
                        position == TagDropPosition::After,
                    ) {
                        this.show_notice(this.error_message(error), cx);
                    }
                }
                this.notify_dialog(cx);
                cx.notify();
            }))
            .when_some(position, |row, position| {
                let line =
                    kit::drop_line(p).debug_selector(|| format!("group-drop-line-{scope:?}"));
                row.child(match position {
                    TagDropPosition::Before => line.top(px(-2.)),
                    TagDropPosition::After => line.bottom(px(-2.)),
                })
            })
    }

    pub(super) fn small_choice<F>(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        cx: &mut Context<Self>,
        apply: F,
    ) -> Stateful<Div>
    where
        F: Fn(&mut Self) + 'static,
    {
        let label: SharedString = label.into();
        kit::segment(self.palette(), id, label, active).on_click(cx.listener(
            move |this, _, window, cx| {
                apply(this);
                this.apply_appearance(window, cx);
            },
        ))
    }

    /// Re-theme the app after the colour mode or style changed, and save it.
    pub(super) fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match self.model.library.config.theme {
            Theme::Light => ComponentThemeMode::Light,
            Theme::Dark => ComponentThemeMode::Dark,
            Theme::System if self.shell.system_dark => ComponentThemeMode::Dark,
            Theme::System => ComponentThemeMode::Light,
        };
        apply_component_theme(
            mode,
            &self.model.library.config.theme_style,
            Some(window),
            cx,
        );
        let _ = self.model.library.save();
        self.sync_spinner_palette(cx);
        cx.notify();
    }

    pub(super) fn tag_management_row(
        &self,
        scope: TagScope,
        tag_id: TagId,
        name: String,
        is_child: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.tags_flow.edit == Some(TagEdit::Rename(tag_id)) {
            return self.tag_inline_editor(format!("rename-tag-{tag_id}"), is_child, cx);
        }
        let p = self.palette();
        let group: SharedString = format!("manage-tag-row-{tag_id}").into();
        let target_parent = self.tags_for(scope).tag(tag_id).and_then(|tag| tag.parent);
        let drop_position = cx
            .has_active_drag()
            .then_some(self.tags_flow.drop_target)
            .flatten()
            .filter(|target| target.scope == scope && target.tag_id == tag_id)
            .map(|target| target.position);
        let drag = TagDrag {
            scope,
            parent: target_parent,
            tag_id,
            name: name.clone(),
        };
        let drag_handle = div()
            .id(ElementId::Name(format!("drag-tag-{tag_id}").into()))
            .absolute()
            .left(px(if is_child { 18. } else { 0. }))
            .top(px(5.))
            .w(px(24.))
            .h(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::OpenHand)
            .opacity(0.)
            .tab_index(0)
            .focus_visible(|handle| handle.opacity(1.))
            .group_hover(group.clone(), |handle| handle.opacity(1.))
            .child(kit::grip(p))
            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()));
        let mut actions = div()
            .w(px(h_sm() * 2. + 2.))
            .h(px(h_sm()))
            .invisible()
            .group_hover(group.clone(), |actions| actions.visible())
            .flex()
            .items_center()
            .justify_end()
            .gap(px(2.));
        if !is_child {
            actions = actions.child(
                kit::icon_button(
                    p,
                    ElementId::Name(format!("add-child-tag-{tag_id}").into()),
                    "icons/plus.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_tag_edit(TagEdit::CreateChild(tag_id), window, cx);
                })),
            );
        }
        actions = actions.child(
            kit::icon_button(
                p,
                ElementId::Name(format!("delete-tag-{tag_id}").into()),
                "icons/trash.svg",
                kit::IconKind::Danger,
                kit::Size::Sm,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tags_flow.delete_pending = Some(tag_id);
                this.tags_flow.edit = None;
                this.tags_flow.error = None;
                this.notify_dialog(cx);
            })),
        );
        kit::list_row(
            p,
            ElementId::Name(format!("manage-tag-{tag_id}").into()),
            false,
        )
        .w_auto()
        .group(group)
        .h(px(row_1() + 4.))
        .ml(px(-24.))
        .pl(px(if is_child { 42. } else { 24. }))
        .pr(px(6.))
        .gap(px(4.))
        .cursor_default()
        .relative()
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<TagDrag>, _, cx| {
                let (drag_scope, drag_parent, drag_id) = {
                    let drag = event.drag(cx);
                    (drag.scope, drag.parent, drag.tag_id)
                };
                let position = if event.bounds.contains(&event.event.position)
                    && drag_scope == scope
                    && drag_parent == target_parent
                    && drag_id != tag_id
                {
                    Some(if event.event.position.y < event.bounds.center().y {
                        TagDropPosition::Before
                    } else {
                        TagDropPosition::After
                    })
                } else {
                    None
                };
                if let Some(position) = position {
                    let target = TagDropTarget {
                        scope,
                        tag_id,
                        position,
                    };
                    if this.tags_flow.drop_target != Some(target) {
                        this.tags_flow.drop_target = Some(target);
                        this.notify_dialog(cx);
                    }
                } else if this
                    .tags_flow
                    .drop_target
                    .is_some_and(|target| target.scope == scope && target.tag_id == tag_id)
                {
                    this.tags_flow.drop_target = None;
                    this.notify_dialog(cx);
                }
            }),
        )
        .on_drop(cx.listener(move |this, drag: &TagDrag, _, cx| {
            let target = this.tags_flow.drop_target.take();
            if drag.scope != scope || drag.parent != target_parent {
                this.notify_dialog(cx);
                return;
            }
            let Some(target) =
                target.filter(|target| target.scope == scope && target.tag_id == tag_id)
            else {
                this.notify_dialog(cx);
                return;
            };
            let moved = match target.position {
                TagDropPosition::Before => {
                    this.tags_for_mut(scope).move_before(drag.tag_id, tag_id)
                }
                TagDropPosition::After => this.tags_for_mut(scope).move_after(drag.tag_id, tag_id),
            };
            if moved {
                this.persist_tags();
            }
            this.notify_dialog(cx);
        }))
        .when_some(drop_position, |row, position| {
            let line = kit::drop_line(p);
            row.child(match position {
                TagDropPosition::Before => line.top(px(-2.)),
                TagDropPosition::After => line.bottom(px(-2.)),
            })
        })
        .child(drag_handle)
        .child(
            div()
                .id(ElementId::Name(format!("edit-tag-name-{tag_id}").into()))
                .min_w_0()
                .flex_1()
                .font_family(mono())
                .text_size(px(fs_body()))
                .text_color(if is_child { p.secondary } else { p.text })
                .cursor_pointer()
                .truncate()
                .child(format!("#{name}"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_tag_edit(TagEdit::Rename(tag_id), window, cx);
                })),
        )
        .child(actions)
        .into_any_element()
    }

    pub(super) fn tag_inline_editor(
        &self,
        id: String,
        is_child: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let can_save = !self.tags_flow.name_input.read(cx).value().trim().is_empty();
        div()
            .id(ElementId::Name(id.into()))
            .min_h(px(row_1() + 4.))
            .pl(px(if is_child { 18. } else { 0. }))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                kit::input(p, &self.tags_flow.name_input)
                    .small()
                    .h(px(h_sm()))
                    .min_w_0()
                    .flex_1()
                    .pl(px(0.))
                    .prefix(div().font_family(mono()).text_color(p.muted).child("#")),
            )
            .child(
                kit::icon_button(
                    p,
                    "cancel-inline-tag-edit",
                    "icons/x.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tags_flow.edit = None;
                    this.tags_flow.error = None;
                    this.notify_dialog(cx);
                })),
            )
            .child(
                kit::icon_button(
                    p,
                    "save-inline-tag-edit",
                    "icons/check.svg",
                    kit::IconKind::Ghost,
                    kit::Size::Sm,
                )
                .when(!can_save, |button| button.opacity(0.4).cursor_default())
                .when(can_save, |button| {
                    button.on_click(cx.listener(|this, _, _, cx| this.commit_tag_edit(cx)))
                }),
            )
            .into_any_element()
    }

    pub(super) fn tag_management_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let scope = self.tags_flow.scope;
        let tags = self.tags_for(scope);
        let roots = tags
            .roots()
            .map(|tag| (tag.id, tag.name.clone()))
            .collect::<Vec<_>>();
        let mut rows = div().flex().flex_col();
        if self.tags_flow.edit == Some(TagEdit::CreateRoot) {
            rows = rows.child(self.tag_inline_editor("new-root-tag-row".into(), false, cx));
        }
        if roots.is_empty() {
            rows = rows.when(self.tags_flow.edit != Some(TagEdit::CreateRoot), |rows| {
                rows.child(
                    div()
                        .h(px(44.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(p.muted)
                        .text_size(px(fs_body()))
                        .child(t!("common.no_tags_yet").to_string()),
                )
            });
        }
        for (id, name) in roots {
            rows = rows.child(self.tag_management_row(scope, id, name, false, cx));
            if self.tags_flow.edit == Some(TagEdit::CreateChild(id)) {
                rows =
                    rows.child(self.tag_inline_editor(format!("new-child-tag-row-{id}"), true, cx));
            }
            for child in tags.children(id) {
                rows = rows.child(self.tag_management_row(
                    scope,
                    child.id,
                    child.name.clone(),
                    true,
                    cx,
                ));
            }
        }
        rows = rows.child(
            kit::button(
                p,
                "new-tag-at-end",
                kit::ButtonKind::Quiet,
                kit::Size::Sm,
                true,
            )
            .mt(px(6.))
            .ml(px(-10.))
            .self_start()
            .child(Self::icon("icons/plus.svg", icon_sm(), p.secondary))
            .child(t!("organize.new_tag").to_string())
            .on_click(cx.listener(|this, _, window, cx| {
                this.start_tag_edit(TagEdit::CreateRoot, window, cx);
            })),
        );
        let delete_panel = self.tags_flow.delete_pending.and_then(|id| {
            tags.tag(id).map(|tag| {
                let count = tags.count(id);
                let has_children = tags.children(id).next().is_some();
                let message = if has_children {
                    crate::i18n::tn(
                        "tags.delete_children_confirm",
                        count,
                        &[
                            ("name", (tag.name).to_string()),
                            ("count", (count).to_string()),
                        ],
                    )
                } else {
                    match scope {
                        TagScope::Skills => crate::i18n::tn(
                            "tags.delete_skills_confirm",
                            count,
                            &[
                                ("name", (tag.name).to_string()),
                                ("count", (count).to_string()),
                            ],
                        ),
                    }
                };
                kit::danger_note(p).mt(px(12.)).child(message).child(
                    div()
                        .mt(px(10.))
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            kit::button(
                                p,
                                "cancel-delete-tag",
                                kit::ButtonKind::Ghost,
                                kit::Size::Sm,
                                true,
                            )
                            .child(t!("common.cancel").to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tags_flow.delete_pending = None;
                                this.notify_dialog(cx);
                            })),
                        )
                        .child(
                            kit::button(
                                p,
                                "confirm-delete-tag",
                                kit::ButtonKind::Danger,
                                kit::Size::Sm,
                                true,
                            )
                            .child(t!("common.delete").to_string())
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.tags_for_mut(scope).delete(id);
                                    this.persist_tags();
                                    let selected = this.tag_filter_for(scope);
                                    if selected == Some(id)
                                        || selected.is_some_and(|selected| {
                                            this.tags_for(scope).tag(selected).is_none()
                                        })
                                    {
                                        this.set_tag_filter(scope, None);
                                    }
                                    this.tags_flow.delete_pending = None;
                                    this.notify_dialog(cx);
                                    cx.notify();
                                },
                            )),
                        ),
                )
            })
        });
        let title = match scope {
            TagScope::Skills => t!("organize.skill_tags").to_string(),
        };
        let close = kit::close_button(p, "close-tags-modal", true)
            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)));
        kit::modal_panel(p)
            .child(kit::modal_header(title, close))
            .child(
                div()
                    .id("tag-manager-scroll")
                    .pl(px(32.))
                    .pr(px(20.))
                    .pt(px(4.))
                    .pb(px(20.))
                    .max_h(px(600.))
                    .overflow_y_scroll()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.tags_flow.edit.is_some() {
                                this.tags_flow.edit = None;
                                this.tags_flow.error = None;
                                this.notify_dialog(cx);
                            }
                        }),
                    )
                    .when_some(delete_panel, |panel, delete| panel.child(delete))
                    .child(rows)
                    .when_some(self.tags_flow.error.clone(), |panel, error| {
                        panel.child(kit::field_error(p, error).mt(px(8.)))
                    }),
            )
    }

    pub(super) fn tag_assignment_row(
        &self,
        scope: TagScope,
        keys: &[String],
        tag_id: TagId,
        name: String,
        is_child: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let checked = !keys.is_empty()
            && keys
                .iter()
                .all(|key| self.tags_for(scope).is_assigned(key, tag_id));
        let mixed = !checked
            && keys
                .iter()
                .any(|key| self.tags_for(scope).is_assigned(key, tag_id));
        let keys = keys.to_vec();
        kit::list_row(
            p,
            ElementId::Name(format!("dialog-assign-tag-{tag_id}").into()),
            false,
        )
        .h(px(row_1()))
        .pl(px(if is_child { 32. } else { 8. }))
        .gap(px(0.))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .font_family(mono())
                .text_size(px(fs_body()))
                .text_color(if is_child { p.secondary } else { p.text })
                .truncate()
                .child(format!("#{name}")),
        )
        .when(checked, |row| {
            row.child(Self::icon("icons/check.svg", icon_sm(), p.text))
        })
        .when(mixed, |row| {
            row.child(
                div()
                    .w(px(14.))
                    .text_center()
                    .text_color(p.muted)
                    .child("—"),
            )
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            let add = !keys
                .iter()
                .all(|key| this.tags_for(scope).is_assigned(key, tag_id));
            for key in &keys {
                let assigned = this.tags_for(scope).is_assigned(key, tag_id);
                if assigned != add {
                    this.tags_for_mut(scope).toggle_assignment(key, tag_id);
                }
            }
            this.persist_tags();
            this.notify_dialog(cx);
            cx.notify();
        }))
        .into_any_element()
    }

    pub(super) fn tag_assignment_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let scope = self.tags_flow.scope;
        let keys = self.tags_flow.assignment_keys.clone();
        let label = self
            .tags_flow
            .assignment_label
            .clone()
            .unwrap_or_else(|| keys.join(", "));
        let tags = self.tags_for(scope);
        let tag_rows = tags
            .tags()
            .iter()
            .map(|tag| (tag.id, tag.name.clone(), tag.parent))
            .collect::<Vec<_>>();
        let mut rows = div().flex().flex_col();
        if tag_rows.is_empty() {
            rows = rows.child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(fs_body()))
                    .text_color(p.muted)
                    .child(t!("common.no_tags_yet").to_string()),
            );
        }
        for (id, name, parent) in tag_rows {
            rows =
                rows.child(self.tag_assignment_row(scope, &keys, id, name, parent.is_some(), cx));
        }
        let close = kit::close_button(p, "close-tag-assignment", true)
            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)));
        kit::modal_panel(p)
            .child(kit::modal_header(t!("common.set_tags").to_string(), close))
            .child(
                div()
                    .px(px(20.))
                    .mt(px(2.))
                    .font_family(mono())
                    .text_size(px(fs_small()))
                    .text_color(p.muted)
                    .truncate()
                    .child(label),
            )
            .child(
                kit::modal_body()
                    .id("tag-assignment-scroll")
                    .pt(px(12.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .child(rows)
                    .child(
                        kit::button(
                            p,
                            "new-tag-from-assignment",
                            kit::ButtonKind::Quiet,
                            kit::Size::Sm,
                            true,
                        )
                        .mt(px(6.))
                        .child(Self::icon("icons/plus.svg", icon_sm(), p.secondary))
                        .child(t!("organize.new_tag").to_string())
                        .on_click(
                            cx.listener(|this, _, _, cx| {
                                this.open_tag_creation_from_assignment(cx)
                            }),
                        ),
                    ),
            )
    }

    pub(super) fn move_group_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let storage_names = self.groups_flow.move_skills.clone();
        let group_ids = storage_names
            .iter()
            .filter_map(|storage_name| {
                self.model
                    .skills
                    .iter()
                    .find(|skill| skill_storage_name(skill) == storage_name)
                    .map(|skill| skill.record.group_id.clone())
            })
            .collect::<Vec<_>>();
        let same_group = group_ids
            .first()
            .is_some_and(|first| group_ids.iter().all(|group| group == first));
        let current_group = same_group
            .then(|| group_ids.first().cloned().flatten())
            .flatten();
        let mixed_group = !same_group && !group_ids.is_empty();
        let skill_name = if storage_names.len() == 1 {
            storage_names
                .first()
                .and_then(|storage_name| {
                    self.model
                        .skills
                        .iter()
                        .find(|skill| skill_storage_name(skill) == storage_name)
                        .map(|skill| skill.record.name.clone())
                })
                .unwrap_or_else(|| storage_names.first().cloned().unwrap_or_default())
        } else {
            crate::i18n::tn(
                "common.skill_count",
                storage_names.len(),
                &[("count", (storage_names.len()).to_string())],
            )
        };
        let groups = self.model.library.groups();
        let mut rows = div().mt(px(12.)).flex().flex_col().gap(px(2.)).mx(px(-8.));
        let no_group_selected = !mixed_group && current_group.is_none();
        rows = rows.child(
            kit::choice_row(
                p,
                "move-group-none",
                t!("organize.no_group").to_string(),
                no_group_selected,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.move_selected_skill_to_group(None, cx);
            })),
        );
        for group in groups {
            let group_id = group.id.clone();
            let selected = current_group.as_deref() == Some(group_id.as_str());
            rows = rows.child(
                kit::choice_row(
                    p,
                    ElementId::Name(format!("move-group-{}", group.id).into()),
                    group.name,
                    selected,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.move_selected_skill_to_group(Some(group_id.clone()), cx);
                })),
            );
        }
        let close = kit::close_button(p, "close-move-group", true)
            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)));
        kit::modal_panel(p)
            .child(kit::modal_header(
                t!("common.move_to_group").to_string(),
                close,
            ))
            .child(
                kit::modal_body()
                    .id("move-group-scroll")
                    .pt(px(2.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .text_color(p.muted)
                            .truncate()
                            .child(skill_name),
                    )
                    .when(mixed_group, |body| {
                        body.child(
                            div()
                                .mt(px(2.))
                                .text_size(px(fs_small()))
                                .text_color(p.muted)
                                .child(t!("organize.current_groups_differ").to_string()),
                        )
                    })
                    .child(rows),
            )
    }

    pub(super) fn add_group_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        if self.groups_flow.edit == Some(GroupEdit::Create) {
            return kit::field(
                p,
                t!("common.group").to_string(),
                self.group_inline_editor("add-group-editor", cx),
            )
            .when_some(self.tags_flow.error.clone(), |view, error| {
                view.child(kit::field_error(p, error))
            })
            .into_any_element();
        }
        let app = cx.entity().downgrade();
        let groups = self.model.library.groups();
        let current = if self.add_flow.group_enabled {
            self.add_flow
                .group_name
                .clone()
                .unwrap_or_else(|| t!("organize.choose_group").to_string())
        } else {
            t!("organize.no_group").to_string()
        };
        let selected_group = self
            .add_flow
            .group_enabled
            .then(|| self.add_flow.group_name.clone())
            .flatten();
        let mut group_options = groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<_>>();
        if let Some(default_group) = selected_group.clone()
            && !group_options
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&default_group))
        {
            group_options.push(default_group);
        }
        let create_group_label = t!("organize.new_group").to_string();
        let group_menu = Popover::new("add-group-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                kit::select_trigger(p, "add-group-select", current)
                    .w_full()
                    .disabled(self.add_flow.task.is_some())
                    .when(self.add_flow.task.is_some(), |button| button.opacity(0.55)),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(240.));
                let clear_app = app.clone();
                menu = menu.child(
                    kit::menu_item(
                        p,
                        "add-group-none",
                        t!("organize.no_group").to_string(),
                        selected_group.is_none(),
                    )
                    .on_click(popover_cx.listener(move |_, _, _, cx| {
                        let _ = clear_app.update(cx, |this, cx| {
                            if this.add_flow.task.is_some() {
                                return;
                            }
                            this.add_flow.group_enabled = false;
                            this.notify_dialog(cx);
                        });
                        cx.emit(DismissEvent);
                    })),
                );
                for (index, group_name) in group_options.clone().into_iter().enumerate() {
                    let item_app = app.clone();
                    let selected = selected_group.as_deref() == Some(group_name.as_str());
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("add-group-option-{index}").into()),
                            group_name.clone(),
                            selected,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = item_app.update(cx, |this, cx| {
                                    if this.add_flow.task.is_some() {
                                        return;
                                    }
                                    this.add_flow.group_enabled = true;
                                    this.add_flow.group_name = Some(group_name.clone());
                                    this.notify_dialog(cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                let create_app = app.clone();
                menu.child(kit::menu_separator(p)).child(
                    kit::menu_action(
                        p,
                        "add-group-create",
                        "icons/plus.svg",
                        create_group_label.clone(),
                    )
                    .on_click(popover_cx.listener(move |_, _, window, cx| {
                        let _ = create_app.update(cx, |this, cx| {
                            if this.add_flow.task.is_some() {
                                return;
                            }
                            this.start_group_edit(GroupEdit::Create, window, cx);
                        });
                        cx.emit(DismissEvent);
                    })),
                )
            });
        div()
            .id("add-group-control")
            .debug_selector(|| "add-group-control".into())
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(kit::field_label(p, t!("common.group").to_string()))
            .child(group_menu)
            .into_any_element()
    }
}
