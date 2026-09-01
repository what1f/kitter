use super::*;

impl KitterApp {
    pub(super) fn small_choice<F>(
        &self,
        id: &'static str,
        label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        apply: F,
    ) -> Stateful<Div>
    where
        F: Fn(&mut Self) + 'static,
    {
        let p = self.palette();
        div()
            .id(id)
            .h(px(24.))
            .px(px(10.))
            .rounded_full()
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(if active { p.selected } else { rgba(0x00000000) })
            .hover(move |choice| choice.bg(p.hover))
            .text_size(px(13.))
            .text_color(if active { p.text } else { p.secondary })
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| {
                apply(this);
                let mode = match this.model.library.config.theme {
                    Theme::Light => ComponentThemeMode::Light,
                    Theme::Dark => ComponentThemeMode::Dark,
                    Theme::System if this.shell.system_dark => ComponentThemeMode::Dark,
                    Theme::System => ComponentThemeMode::Light,
                };
                ComponentTheme::change(mode, Some(window), cx);
                let _ = this.model.library.save();
                this.sync_spinner_palette(cx);
                cx.notify();
            }))
    }

    pub(super) fn tag_management_row(
        &self,
        scope: TagScope,
        tag_id: TagId,
        name: String,
        is_child: bool,
        count: usize,
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
        let dot_column = || {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().size(px(3.)).rounded_full().bg(p.muted))
                .child(div().size(px(3.)).rounded_full().bg(p.muted))
                .child(div().size(px(3.)).rounded_full().bg(p.muted))
        };
        let drag_handle = div()
            .id(ElementId::Name(format!("drag-tag-{tag_id}").into()))
            .w(px(16.))
            .h(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::OpenHand)
            .invisible()
            .group_hover(group.clone(), |handle| handle.visible())
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .child(dot_column())
                    .child(dot_column()),
            )
            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()));
        let mut actions = div()
            .w(px(52.))
            .h(px(24.))
            .invisible()
            .group_hover(group.clone(), |actions| actions.visible())
            .flex()
            .items_center()
            .justify_end()
            .gap(px(2.));
        if !is_child {
            actions = actions.child(
                div()
                    .id(ElementId::Name(format!("add-child-tag-{tag_id}").into()))
                    .size(px(24.))
                    .rounded(px(7.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |button| button.bg(p.hover))
                    .child(Self::icon("icons/plus.svg", 13., p.muted))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_tag_edit(TagEdit::CreateChild(tag_id), window, cx);
                    })),
            );
        }
        actions = actions.child(
            div()
                .id(ElementId::Name(format!("delete-tag-{tag_id}").into()))
                .size(px(24.))
                .rounded(px(7.5))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |button| button.bg(p.danger_soft))
                .child(Self::icon("icons/x.svg", 13., p.danger))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tags_flow.delete_pending = Some(tag_id);
                    this.tags_flow.edit = None;
                    this.tags_flow.error = None;
                    this.notify_dialog(cx);
                })),
        );
        div()
            .id(ElementId::Name(format!("manage-tag-{tag_id}").into()))
            .group(group)
            .h(px(34.))
            .pl(px(if is_child { 18. } else { 0. }))
            .pr(px(6.))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .gap(px(4.))
            .hover(move |row| row.bg(p.hover))
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
                    TagDropPosition::After => {
                        this.tags_for_mut(scope).move_after(drag.tag_id, tag_id)
                    }
                };
                if moved {
                    this.persist_tags();
                }
                this.notify_dialog(cx);
            }))
            .when_some(drop_position, |row, position| {
                let line = div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h(px(2.))
                    .rounded_full()
                    .bg(p.accent);
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
                    .font_family(MONO)
                    .text_size(px(13.))
                    .text_color(if is_child { p.secondary } else { p.text })
                    .cursor_pointer()
                    .truncate()
                    .child(format!("#{name}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_tag_edit(TagEdit::Rename(tag_id), window, cx);
                    })),
            )
            .child(
                div()
                    .w(px(24.))
                    .flex()
                    .justify_end()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .child(count.to_string()),
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
            .min_h(px(34.))
            .pl(px(if is_child { 18. } else { 0. }))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().w(px(16.)))
            .child(
                Input::new(&self.tags_flow.name_input)
                    .small()
                    .h(px(30.))
                    .min_w_0()
                    .flex_1()
                    .rounded(px(RADIUS_INLINE_INPUT))
                    .bg(p.surface)
                    .border_color(p.border_strong)
                    .pl(px(0.))
                    .text_size(px(13.))
                    .prefix(div().font_family(MONO).text_color(p.muted).child("#")),
            )
            .child(
                div()
                    .id("cancel-inline-tag-edit")
                    .size(px(24.))
                    .rounded(px(7.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |button| button.bg(p.hover))
                    .child(Self::icon("icons/x.svg", 13., p.muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tags_flow.edit = None;
                        this.tags_flow.error = None;
                        this.notify_dialog(cx);
                    })),
            )
            .child(
                div()
                    .id("save-inline-tag-edit")
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
                            .on_click(cx.listener(|this, _, _, cx| this.commit_tag_edit(cx)))
                    })
                    .child(Self::icon(
                        "icons/check.svg",
                        13.,
                        if can_save { p.text } else { p.muted },
                    )),
            )
            .into_any_element()
    }

    pub(super) fn tag_management_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let scope = self.tags_flow.scope;
        let tags = self.tags_for(scope);
        let roots = tags
            .roots()
            .map(|tag| (tag.id, tag.name.clone(), tags.count(tag.id)))
            .collect::<Vec<_>>();
        let mut rows = div().mt(px(12.)).flex().flex_col();
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
                        .text_size(px(13.))
                        .child(self.tr("还没有标签", "No tags yet")),
                )
            });
        }
        for (id, name, count) in roots {
            rows = rows.child(self.tag_management_row(scope, id, name, false, count, cx));
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
                    tags.count(child.id),
                    cx,
                ));
            }
        }
        rows = rows.child(
            div()
                .id("new-tag-at-end")
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
                .child(self.tr("新建标签", "New tag"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.start_tag_edit(TagEdit::CreateRoot, window, cx);
                })),
        );
        let delete_panel = self.tags_flow.delete_pending.and_then(|id| {
            tags.tag(id).map(|tag| {
                let count = tags.count(id);
                let has_children = tags.children(id).next().is_some();
                let message = if self.uses_english() {
                    format!(
                        "Delete #{}? Child tags and {} assignment(s) will also be removed.",
                        tag.name, count
                    )
                } else if has_children {
                    format!(
                        "删除 #{}？它的子标签和 {} 个关联也会一并移除。",
                        tag.name, count
                    )
                } else {
                    let target = match scope {
                        TagScope::Skills => "技能",
                        TagScope::Projects => "项目",
                    };
                    format!("删除 #{}？{} 个{}将失去这个标签。", tag.name, count, target)
                };
                div()
                    .mt(px(12.))
                    .p(px(12.))
                    .rounded(px(RADIUS_CONTROL))
                    .bg(p.danger_soft)
                    .text_size(px(13.))
                    .child(message)
                    .child(
                        div()
                            .mt(px(10.))
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("cancel-delete-tag")
                                    .h(px(CONTROL_HEIGHT))
                                    .px(px(10.))
                                    .rounded(px(RADIUS_CONTROL))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .hover(move |button| button.bg(p.hover))
                                    .child(self.tr("取消", "Cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tags_flow.delete_pending = None;
                                        this.notify_dialog(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("confirm-delete-tag")
                                    .h(px(CONTROL_HEIGHT))
                                    .px(px(10.))
                                    .rounded(px(RADIUS_CONTROL))
                                    .bg(p.danger)
                                    .text_color(p.on_accent)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .hover(move |button| button.opacity(0.82))
                                    .child(self.tr("删除", "Delete"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
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
                                    })),
                            ),
                    )
            })
        });
        div()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .id("tag-manager-scroll")
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(18.))
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(match scope {
                                        TagScope::Skills => self.tr("技能标签", "Skill tags"),
                                        TagScope::Projects => self.tr("项目标签", "Project tags"),
                                    }),
                            )
                            .child(div().flex_1())
                            .child(div().w(px(6.)))
                            .child(
                                div()
                                    .id("close-tags-modal")
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
                    .when_some(delete_panel, |panel, delete| panel.child(delete))
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
        div()
            .id(ElementId::Name(
                format!("dialog-assign-tag-{tag_id}").into(),
            ))
            .h(px(34.))
            .pl(px(if is_child { 34. } else { 10. }))
            .pr(px(10.))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(move |row| row.bg(p.hover))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .font_family(MONO)
                    .text_size(px(13.))
                    .text_color(if is_child { p.secondary } else { p.text })
                    .truncate()
                    .child(format!("#{name}")),
            )
            .when(checked, |row| {
                row.child(Self::icon("icons/check.svg", 14., p.text))
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
        let mut rows = div().mt(px(12.)).flex().flex_col();
        if tag_rows.is_empty() {
            rows = rows.child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(p.muted)
                    .child(self.tr("还没有标签", "No tags yet")),
            );
        }
        for (id, name, parent) in tag_rows {
            rows =
                rows.child(self.tag_assignment_row(scope, &keys, id, name, parent.is_some(), cx));
        }
        div()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .id("tag-assignment-scroll")
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(18.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(self.tr("设置标签", "Set tags")),
                                    )
                                    .child(
                                        div()
                                            .mt(px(3.))
                                            .font_family(MONO)
                                            .text_size(px(12.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(label),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("close-tag-assignment")
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
                    .child(
                        div()
                            .id("new-tag-from-assignment")
                            .mt(px(8.))
                            .h(px(CONTROL_HEIGHT))
                            .px(px(8.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(p.muted)
                            .hover(move |button| button.bg(p.hover).text_color(p.text))
                            .child(self.tr("＋ 新建标签", "+ New tag"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_tag_creation_from_assignment(cx)
                            })),
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
        } else if self.uses_english() {
            format!("{} Skills", storage_names.len())
        } else {
            format!("{} 个技能", storage_names.len())
        };
        let groups = self.model.library.groups();
        let mut rows = div().mt(px(12.)).flex().flex_col().gap(px(2.));
        let no_group_selected = !mixed_group && current_group.is_none();
        rows = rows.child(
            div()
                .id("move-group-none")
                .h(px(36.))
                .px(px(10.))
                .rounded(px(5.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(13.))
                .bg(if no_group_selected {
                    p.selected
                } else {
                    rgba(0x00000000)
                })
                .hover(move |row| row.bg(p.hover))
                .child(self.tr("不使用分组", "No group"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.move_selected_skill_to_group(None, cx);
                })),
        );
        for group in groups {
            let group_id = group.id.clone();
            let selected = current_group.as_deref() == Some(group_id.as_str());
            rows = rows.child(
                div()
                    .id(ElementId::Name(format!("move-group-{}", group.id).into()))
                    .h(px(36.))
                    .px(px(10.))
                    .rounded(px(5.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .bg(if selected {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |row| row.bg(p.hover))
                    .child(group.name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.move_selected_skill_to_group(Some(group_id.clone()), cx);
                    })),
            );
        }
        div()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .id("move-group-scroll")
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(20.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.tr("移动分组", "Move to group")),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("close-move-group")
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
                    .child(
                        div()
                            .mt(px(6.))
                            .font_family(MONO)
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .truncate()
                            .child(skill_name),
                    )
                    .when(mixed_group, |body| {
                        body.child(
                            div()
                                .mt(px(3.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(self.tr("当前分组不一致", "Current groups differ")),
                        )
                    })
                    .child(rows),
            )
    }

    pub(super) fn add_group_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let english = self.uses_english();
        let app = cx.entity().downgrade();
        let groups = self.model.library.groups();
        let current = if self.add_flow.group_enabled {
            self.add_flow
                .group_name
                .clone()
                .unwrap_or_else(|| self.tr("选择分组", "Choose group").to_string())
        } else {
            self.tr("不使用分组", "No group").to_string()
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
        let create_group_label = if english {
            "+ New group"
        } else {
            "＋ 新建分组"
        };
        let group_menu = Popover::new("add-group-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                self.dropdown_button("add-group-select", current, 13.)
                    .w_full(),
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
                let clear_app = app.clone();
                menu = menu.child(
                    div()
                        .id("add-group-none")
                        .h(px(32.))
                        .px(px(10.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(p.text)
                        .bg(if selected_group.is_none() {
                            p.selected
                        } else {
                            rgba(0x00000000)
                        })
                        .hover(move |row| row.bg(p.hover))
                        .child(if english {
                            "No group"
                        } else {
                            "不使用分组"
                        })
                        .on_click(popover_cx.listener(move |_, _, _, cx| {
                            let _ = clear_app.update(cx, |this, cx| {
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
                        div()
                            .id(ElementId::Name(format!("add-group-option-{index}").into()))
                            .h(px(32.))
                            .px(px(10.))
                            .rounded(px(5.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(p.text)
                            .bg(if selected {
                                p.selected
                            } else {
                                rgba(0x00000000)
                            })
                            .hover(move |row| row.bg(p.hover))
                            .child(group_name.clone())
                            .on_click(popover_cx.listener(move |_, _, _, cx| {
                                let _ = item_app.update(cx, |this, cx| {
                                    this.add_flow.group_enabled = true;
                                    this.add_flow.group_name = Some(group_name.clone());
                                    this.notify_dialog(cx);
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                let create_app = app.clone();
                menu.child(
                    div()
                        .id("add-group-create")
                        .mt(px(4.))
                        .pt(px(4.))
                        .border_t_1()
                        .border_color(p.border)
                        .h(px(32.))
                        .px(px(10.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(p.secondary)
                        .hover(move |row| row.bg(p.hover))
                        .child(create_group_label)
                        .on_click(popover_cx.listener(move |_, _, window, cx| {
                            let _ = create_app.update(cx, |this, cx| {
                                this.open_group_dialog(cx);
                                this.start_group_edit(GroupEdit::Create, window, cx);
                            });
                            cx.emit(DismissEvent);
                        })),
                )
            });
        div()
            .id("add-group-control")
            .mt(px(5.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(self.tr("分组", "Group")),
            )
            .child(group_menu)
            .into_any_element()
    }
}
