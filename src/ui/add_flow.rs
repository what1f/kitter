use super::*;

impl KitterApp {
    pub(super) fn add_skill_modal(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let pending = self.add_flow.task.is_some();
        let selected_kind = self.add_flow.kind;
        let source_label = match selected_kind {
            AddKind::Npx => "skills.sh / github",
            AddKind::Claude => self.tr("Claude 插件", "Claude plugin"),
            AddKind::Local => self.tr("本地文件夹", "Local folder"),
            AddKind::Existing => self.tr("已有安装", "Existing installations"),
        };
        let app = cx.entity().downgrade();
        let english = self.uses_english();
        let source_control = Popover::new("add-source-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                self.dropdown_button("add-source-select", source_label, 14.)
                    .w_full()
                    .disabled(pending),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = div()
                    .w(px(486.))
                    .p(px(4.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.elevated)
                    .shadow_lg();
                for (index, (kind, label)) in [
                    (AddKind::Npx, "skills.sh / github"),
                    (
                        AddKind::Claude,
                        if english {
                            "Claude plugin"
                        } else {
                            "Claude 插件"
                        },
                    ),
                    (
                        AddKind::Local,
                        if english {
                            "Local folder"
                        } else {
                            "本地文件夹"
                        },
                    ),
                    (
                        AddKind::Existing,
                        if english {
                            "Existing installations"
                        } else {
                            "已有安装"
                        },
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let active = selected_kind == kind;
                    let app = app.clone();
                    menu = menu.child(
                        div()
                            .id(ElementId::Name(format!("add-source-option-{index}").into()))
                            .h(px(29.))
                            .px(px(10.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(if active { p.selected } else { rgba(0x00000000) })
                            .hover(move |item| item.bg(p.hover))
                            .font_family(FONT_UI)
                            .text_size(px(13.))
                            .child(label)
                            .on_click(popover_cx.listener(move |_, _, window, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.set_add_kind(kind, window, cx);
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                menu
            });
        let mut form = div().mt(px(14.)).flex().flex_col().gap(px(7.));
        form = form
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(self.tr("从哪里添加", "Source")),
            )
            .child(source_control);
        if self.add_flow.selected.len() > 1 && self.add_flow.kind != AddKind::Existing {
            form = form.child(self.add_group_control(cx));
        }
        if self.add_flow.kind == AddKind::Existing {
            let root_label = self
                .add_flow
                .adoption_root
                .as_deref()
                .map(display_path)
                .unwrap_or_else(|| self.tr("用户目录", "Home folder").into());
            form = form.child(
                div()
                    .mt(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("choose-adoption-root")
                            .flex_1()
                            .min_w_0()
                            .h(px(36.))
                            .px(px(11.))
                            .rounded(px(RADIUS_CONTROL))
                            .border_1()
                            .border_color(p.border_strong)
                            .bg(p.surface)
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(Self::icon("icons/folder.svg", 15., p.secondary))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.))
                                    .child(root_label),
                            )
                            .when(!pending, |button| {
                                button.cursor_pointer().on_click(
                                    cx.listener(|this, _, _, cx| this.browse_adoption_root(cx)),
                                )
                            }),
                    )
                    .when(self.add_flow.adoption_root.is_some() && !pending, |row| {
                        row.child(
                            div()
                                .id("reset-adoption-root")
                                .text_size(px(12.))
                                .text_color(p.secondary)
                                .cursor_pointer()
                                .child(self.tr("重置", "Reset"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.add_flow.adoption_root = None;
                                    this.add_flow.adoption_scan = None;
                                    this.add_flow.selected.clear();
                                    this.notify_dialog(cx);
                                })),
                        )
                    }),
            );
        }
        if self.add_flow.kind == AddKind::Local {
            form = form.child(
                div()
                    .id("choose-local-skill")
                    .debug_selector(|| "choose-local-skill".into())
                    .mt(px(5.))
                    .h(px(40.))
                    .px(px(11.))
                    .rounded(px(RADIUS_CONTROL))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.surface)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor(if pending {
                        CursorStyle::Arrow
                    } else {
                        CursorStyle::PointingHand
                    })
                    .child(Self::icon("icons/folder.svg", 16., p.secondary))
                    .text_size(px(14.))
                    .child(self.tr("选择技能文件夹", "Choose Skill folder"))
                    .when(!pending, |button| {
                        button.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.browse_add_local(window, cx)),
                        )
                    }),
            );
        } else if self.add_flow.kind != AddKind::Existing {
            form = form
                .child(
                    div()
                        .mt(px(5.))
                        .text_size(px(13.))
                        .text_color(p.secondary)
                        .child(self.tr("地址", "Address")),
                )
                .child(
                    Input::new(&self.add_flow.primary_input)
                        .h(px(INPUT_HEIGHT))
                        .w_full()
                        .rounded(px(RADIUS_CONTROL))
                        .bg(p.surface)
                        .border_color(p.border_strong)
                        .text_size(px(14.)),
                );
        }
        if let Some(error) = &self.add_flow.error {
            form = form.child(
                div()
                    .mt(px(4.))
                    .text_size(px(12.))
                    .text_color(p.danger)
                    .child(error.clone()),
            );
        }
        if let Some(scan) = &self.add_flow.adoption_scan {
            let rows = list(
                self.add_flow.adoption_list.clone(),
                cx.processor(|this, index, _, cx| this.adoption_row(index, cx)),
            )
            .h(px(270.))
            .w_full();
            let all_safe = scan.selectable_ids();
            let all_selected = !all_safe.is_empty()
                && all_safe
                    .iter()
                    .all(|id| self.add_flow.selected.contains(id));
            form = form
                .child(
                    div()
                        .mt(px(8.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.secondary)
                                .child(self.tr("选择技能", "Select Skills")),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("adoption-select-all")
                                .text_size(px(12.))
                                .text_color(p.secondary)
                                .child(if all_selected {
                                    self.tr("取消全选", "Clear all")
                                } else {
                                    self.tr("全选", "Select all")
                                })
                                .when(!pending, |toggle| {
                                    toggle.cursor_pointer().on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if all_selected {
                                                this.add_flow.selected.clear();
                                            } else {
                                                if let Some(scan) = &this.add_flow.adoption_scan {
                                                    this.add_flow.selected.extend(
                                                        scan.selectable_ids().iter().cloned(),
                                                    );
                                                }
                                            }
                                            this.notify_dialog(cx);
                                        },
                                    ))
                                }),
                        ),
                )
                .child(
                    div()
                        .rounded(px(RADIUS_CARD))
                        .border_1()
                        .border_color(p.border_strong)
                        .bg(p.surface)
                        .p(px(6.))
                        .overflow_hidden()
                        .child(rows),
                );
        } else if let Some(scan) = &self.add_flow.scan {
            let source_key = scan.source_key();
            let selectable = scan
                .skills()
                .iter()
                .filter(|skill| {
                    !self.model.skills.iter().any(|installed| {
                        installed.record.name == skill.name
                            && installed.record.origin.source().key() == source_key
                    })
                })
                .map(|skill| skill.name.clone())
                .collect::<Vec<_>>();
            let all_selected = !selectable.is_empty()
                && selectable
                    .iter()
                    .all(|skill| self.add_flow.selected.contains(skill));
            let toggle_names = selectable.clone();
            let mut candidate_rows = div()
                .id("scanned-skills-list")
                .flex()
                .flex_col()
                .gap(px(2.))
                .max_h(px(220.))
                .overflow_hidden()
                .overflow_y_scroll()
                .overflow_x_hidden();
            for (index, skill) in scan.skills().iter().enumerate() {
                let name = skill.name.clone();
                let installed = self.model.skills.iter().any(|installed| {
                    installed.record.name == name
                        && installed.record.origin.source().key() == source_key
                });
                let selected = self.add_flow.selected.contains(&name);
                let description = skill.description.clone();
                candidate_rows = candidate_rows.child(
                    div()
                        .id(ElementId::Name(format!("scan-skill-{index}").into()))
                        .debug_selector(move || format!("scan-skill-{index}"))
                        .rounded(px(RADIUS_CONTROL))
                        .min_h(px(52.))
                        .px(px(11.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .bg(if selected { p.selected } else { p.surface })
                        .cursor(if installed || pending {
                            CursorStyle::Arrow
                        } else {
                            CursorStyle::PointingHand
                        })
                        .child(
                            Checkbox::new(ElementId::Name(format!("scan-checkbox-{name}").into()))
                                .checked(selected),
                        )
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
                                .when(!description.is_empty(), |column| {
                                    column.child(
                                        div()
                                            .mt(px(2.))
                                            .font_family(MONO)
                                            .text_size(px(12.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(description),
                                    )
                                }),
                        )
                        .when(installed, |row| {
                            row.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(self.tr("已添加", "Added")),
                            )
                        })
                        .when(!installed && !pending, |row| {
                            row.on_click(cx.listener(move |this, _, _, cx| {
                                if !this.add_flow.selected.remove(&name) {
                                    this.add_flow.selected.insert(name.clone());
                                }
                                this.notify_dialog(cx);
                            }))
                        }),
                );
            }
            // GPUI scroll clipping is rectangular: keep the viewport inside the card's corners.
            let candidates = div()
                .p(px(6.))
                .rounded(px(RADIUS_CARD))
                .border_1()
                .border_color(p.border_strong)
                .bg(p.surface);
            let candidates = candidates.overflow_hidden().child(candidate_rows);
            form = form
                .child(
                    div()
                        .mt(px(8.))
                        .flex()
                        .items_center()
                        .child(div().text_size(px(12.)).text_color(p.secondary).child(
                            if self.uses_english() {
                                format!("Found {} Skills", scan.skills().len())
                            } else {
                                format!("发现 {} 个技能", scan.skills().len())
                            },
                        ))
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("toggle-all-scanned-skills")
                                .text_size(px(12.))
                                .text_color(p.accent)
                                .cursor_pointer()
                                .child(if all_selected {
                                    self.tr("取消全选", "Clear all")
                                } else {
                                    self.tr("全选", "Select all")
                                })
                                .when(!pending, |toggle| {
                                    toggle.on_click(cx.listener(move |this, _, _, cx| {
                                        if all_selected {
                                            for name in &toggle_names {
                                                this.add_flow.selected.remove(name);
                                            }
                                        } else {
                                            this.add_flow.selected.extend(toggle_names.clone());
                                        }
                                        this.notify_dialog(cx);
                                    }))
                                }),
                        ),
                )
                .child(candidates)
                .child(
                    div()
                        .font_family(MONO)
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .truncate()
                        .child(display_path(Path::new(scan.source_label()))),
                );
        } else if self.add_flow.task == Some(AddTask::Scanning) {
            form = form.child(
                div()
                    .mt(px(10.))
                    .h(px(82.))
                    .rounded(px(RADIUS_CARD))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(9.))
                    .text_size(px(14.))
                    .text_color(p.secondary)
                    .child(self.shell.spinner_accent.clone())
                    .child(self.tr("正在扫描…", "Scanning…")),
            );
        }
        let has_scan = self.add_flow.scan.is_some() || self.add_flow.adoption_scan.is_some();
        let ready = !pending
            && if has_scan {
                !self.add_flow.selected.is_empty()
            } else if self.add_flow.kind == AddKind::Existing {
                true
            } else if self.add_flow.kind == AddKind::Local {
                false
            } else {
                !self
                    .add_flow
                    .primary_input
                    .read(cx)
                    .value()
                    .trim()
                    .is_empty()
            };
        let action_label = match self.add_flow.task {
            Some(AddTask::Scanning) => self.tr("正在扫描…", "Scanning…").to_string(),
            Some(AddTask::Importing) if self.add_flow.kind == AddKind::Existing => {
                self.tr("正在托管…", "Adopting…").to_string()
            }
            Some(AddTask::Importing) => self.tr("正在添加…", "Adding…").to_string(),
            None if has_scan && self.add_flow.kind == AddKind::Existing => {
                if self.uses_english() {
                    format!("Adopt {}", self.add_flow.selected.len())
                } else {
                    format!("托管 {} 个", self.add_flow.selected.len())
                }
            }
            None if has_scan => {
                if self.uses_english() {
                    format!("Add {}", self.add_flow.selected.len())
                } else {
                    format!("添加 {} 个", self.add_flow.selected.len())
                }
            }
            None if self.add_flow.kind == AddKind::Existing => self.tr("扫描", "Scan").to_string(),
            None => self.tr("检测", "Scan").to_string(),
        };
        let close_button = div()
            .id("close-add-modal")
            .debug_selector(|| "close-add-modal".into())
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
            .when(
                !pending || self.add_flow.adoption_cancel.is_some(),
                |button| {
                    button.on_click(cx.listener(|this, _, _, cx| {
                        this.close_dialog(cx);
                    }))
                },
            );
        let panel = div()
            .relative()
            .bg(p.elevated)
            .rounded(px(RADIUS_MODAL))
            .overflow_hidden()
            .child(
                div()
                    .id("add-modal-content")
                    .rounded_tl(px(RADIUS_MODAL))
                    .rounded_tr(px(RADIUS_MODAL))
                    .bg(p.elevated)
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(19.))
                    .max_h(px(520.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .font_family(FONT_UI)
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.tr("添加技能", "Add Skill")),
                            )
                            .child(div().flex_1())
                            .child(close_button),
                    )
                    .child(
                        div()
                            .mt(px(5.))
                            .text_size(px(14.))
                            .line_height(relative(1.5))
                            .text_color(p.muted)
                            .child(if self.add_flow.kind == AddKind::Existing {
                                self.tr("扫描指定目录下所有技能并托管到 Kitter。", "Scan all skills in the selected directory and adopt them into Kitter.")
                            } else {
                                self.tr("选择来源并粘贴地址，Kitter 会自动识别其中可用的技能。", "Choose a source and paste its address. Kitter will find the available Skills.")
                            }),
                    )
                    .child(form),
            )
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
                            .id("cancel-add")
                            .h(px(DIALOG_CONTROL_HEIGHT))
                            .px(px(16.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor(if pending {
                                CursorStyle::Arrow
                            } else {
                                CursorStyle::PointingHand
                            })
                            .child(self.tr("取消", "Cancel"))
                            .when(!pending || self.add_flow.adoption_cancel.is_some(), |button| {
                                button
                                    .hover(move |button| button.bg(p.hover))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.close_dialog(cx);
                                        }),
                                    )
                            }),
                    )
                    .when(self.add_flow.kind != AddKind::Local || has_scan, |footer| {
                        footer.child(
                            div()
                                .id("confirm-add")
                                .debug_selector(|| "confirm-add".into())
                                .h(px(DIALOG_CONTROL_HEIGHT))
                                .px(px(16.))
                                .rounded(px(RADIUS_CONTROL))
                                .border_1()
                                .border_color(p.border)
                                .bg(if ready { p.accent_fill } else { p.raised })
                                .text_color(if ready { p.on_accent } else { p.muted })
                                .flex()
                                .items_center()
                                .cursor(if ready {
                                    CursorStyle::PointingHand
                                } else {
                                    CursorStyle::Arrow
                                })
                                .gap(px(6.))
                                .when(pending, |button| {
                                    button.child(self.shell.spinner_on_accent.clone())
                                })
                                .child(action_label)
                                .when(ready, |button| {
                                    button
                                        .hover(move |button| button.opacity(0.82))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                if has_scan {
                                                    this.import_scanned_skills(cx);
                                                } else {
                                                    this.scan_add_source(cx);
                                                }
                                            }),
                                        )
                                }),
                        )
                    }),
            );
        let _ = window;
        panel
    }
}
