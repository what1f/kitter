use super::*;

impl KitterApp {
    fn scan_progress(&self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette();
        let label = if matches!(self.add_flow.kind, AddKind::Npx | AddKind::Claude) {
            t!("add.fetching_skills").to_string()
        } else {
            t!("add.scanning_skills").to_string()
        };
        let fill = div()
            .absolute()
            .h_full()
            .rounded(px(r_xs()))
            .bg(p.secondary)
            .left(relative(motion::progress_phase(window, cx) * 1.3 - 0.3))
            .w(relative(0.3));
        div()
            .id("scan-progress")
            .debug_selector(|| "scan-progress".into())
            .mt(px(16.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(fs_small()))
                    .text_color(p.secondary)
                    .child(label),
            )
            .child(kit::track(p).child(fill))
    }

    pub(super) fn add_skill_modal(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let pending = self.add_flow.task.is_some();
        let can_close = !pending || self.add_flow.adoption_cancel.is_some();
        if pending
            && self
                .add_flow
                .primary_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        {
            self.shell.focus_handle.focus(window, cx);
        }
        let selected_kind = self.add_flow.kind;
        let source_label = match selected_kind {
            AddKind::Npx => "skills.sh / github".to_string(),
            AddKind::Claude => t!("common.claude_plugin").to_string(),
            AddKind::Local => t!("common.local_folder").to_string(),
            AddKind::Existing => t!("common.existing_installations").to_string(),
        };
        let app = cx.entity().downgrade();
        let source_control = Popover::new("add-source-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                kit::select_trigger(p, "add-source-select", source_label)
                    .w_full()
                    .disabled(pending)
                    .when(pending, |button| button.opacity(0.55)),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(480.));
                for (index, (kind, label)) in [
                    (AddKind::Npx, "skills.sh / github".to_string()),
                    (AddKind::Claude, t!("common.claude_plugin").to_string()),
                    (AddKind::Local, t!("common.local_folder").to_string()),
                    (
                        AddKind::Existing,
                        t!("common.existing_installations").to_string(),
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let active = selected_kind == kind;
                    let app = app.clone();
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("add-source-option-{index}").into()),
                            label,
                            active,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, window, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.set_add_kind(kind, window, cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            });
        let mut form =
            kit::form().child(kit::field(p, t!("add.source").to_string(), source_control));
        if self.add_flow.selected.len() > 1 && self.add_flow.kind != AddKind::Existing {
            form = form.child(self.add_group_control(cx));
        }
        if self.add_flow.kind == AddKind::Existing {
            let root_label = self
                .add_flow
                .adoption_root
                .as_deref()
                .map(display_path)
                .unwrap_or_else(|| t!("add.home_folder").to_string().into());
            form = form.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        kit::picker(
                            p,
                            "choose-adoption-root",
                            "icons/folder.svg",
                            root_label,
                            !pending,
                        )
                        .flex_1()
                        .when(!pending, |picker| {
                            picker.on_click(
                                cx.listener(|this, _, _, cx| this.browse_adoption_root(cx)),
                            )
                        }),
                    )
                    .when(self.add_flow.adoption_root.is_some() && !pending, |row| {
                        row.child(
                            kit::button(
                                p,
                                "reset-adoption-root",
                                kit::ButtonKind::Quiet,
                                kit::Size::Md,
                                true,
                            )
                            .child(t!("add.reset").to_string())
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
            let mode = self.add_flow.local_mode;
            let mut modes = kit::segmented(p);
            for (id, value, label) in [
                (
                    "local-mode-link",
                    crate::LocalInstallMode::Link,
                    t!("add.local_mode_link"),
                ),
                (
                    "local-mode-copy",
                    crate::LocalInstallMode::Copy,
                    t!("add.local_mode_copy"),
                ),
            ] {
                modes = modes.child(kit::segment(p, id, label.to_string(), mode == value).when(
                    !pending,
                    |segment| {
                        segment.on_click(cx.listener(move |this, _, _, cx| {
                            this.add_flow.local_mode = value;
                            this.notify_dialog(cx);
                        }))
                    },
                ));
            }
            form = form.child(kit::field(
                p,
                t!("add.local_mode").to_string(),
                div().flex().child(modes),
            ));
            form = form.child(
                kit::picker(
                    p,
                    "choose-local-skill",
                    "icons/folder.svg",
                    t!("add.choose_skill_folder").to_string(),
                    !pending,
                )
                .when(!pending, |picker| {
                    picker.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.browse_add_local(window, cx)),
                    )
                }),
            );
        } else if self.add_flow.kind != AddKind::Existing {
            form = form.child(kit::field(
                p,
                t!("add.address").to_string(),
                kit::input(p, &self.add_flow.primary_input).disabled(pending),
            ));
        }
        if let Some(error) = &self.add_flow.error {
            form = form.child(kit::field_error(p, error.clone()));
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
            let toggle = kit::button(
                p,
                "adoption-select-all",
                kit::ButtonKind::Quiet,
                kit::Size::Sm,
                !pending,
            )
            .child(if all_selected {
                t!("add.clear_all").to_string()
            } else {
                t!("common.select_all").to_string()
            })
            .when(!pending, |toggle| {
                toggle.on_click(cx.listener(move |this, _, _, cx| {
                    if all_selected {
                        this.add_flow.selected.clear();
                    } else if let Some(scan) = &this.add_flow.adoption_scan {
                        this.add_flow
                            .selected
                            .extend(scan.selectable_ids().iter().cloned());
                    }
                    this.notify_dialog(cx);
                }))
            });
            form = form.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(kit::field_label(p, t!("add.select_skills").to_string()))
                            .child(toggle),
                    )
                    .child(kit::card(p).p(px(4.)).child(rows)),
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
                        .rounded(px(r_md()))
                        .min_h(px(row_2()))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .selected_fill(p, selected)
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
                                        .font_family(mono())
                                        .text_size(px(fs_body()))
                                        .font_weight(weight_medium())
                                        .truncate()
                                        .child(name.clone()),
                                )
                                .when(!description.is_empty(), |column| {
                                    column.child(
                                        div()
                                            .mt(px(2.))
                                            .text_size(px(fs_small()))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(description),
                                    )
                                }),
                        )
                        .when(installed, |row| {
                            row.child(kit::badge(p).child(t!("add.added").to_string()))
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
            let candidates = kit::card(p).p(px(4.)).child(candidate_rows);
            let toggle = kit::button(
                p,
                "toggle-all-scanned-skills",
                kit::ButtonKind::Quiet,
                kit::Size::Sm,
                !pending,
            )
            .child(if all_selected {
                t!("add.clear_all").to_string()
            } else {
                t!("common.select_all").to_string()
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
            });
            form = form.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(kit::field_label(
                                p,
                                crate::i18n::tn(
                                    "add.found_skills",
                                    scan.skills().len(),
                                    &[("count", (scan.skills().len()).to_string())],
                                ),
                            ))
                            .child(toggle),
                    )
                    .child(candidates)
                    .child(
                        div()
                            .font_family(mono())
                            .text_size(px(fs_caption()))
                            .text_color(p.muted)
                            .truncate()
                            .child(display_path(Path::new(scan.source_label()))),
                    ),
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
            Some(AddTask::Scanning)
                if matches!(self.add_flow.kind, AddKind::Npx | AddKind::Claude) =>
            {
                t!("add.fetching").to_string()
            }
            Some(AddTask::Scanning) => t!("common.scanning").to_string(),
            Some(AddTask::Importing) if self.add_flow.kind == AddKind::Existing => {
                t!("add.adopting").to_string()
            }
            Some(AddTask::Importing) => t!("add.adding").to_string(),
            None if has_scan && self.add_flow.kind == AddKind::Existing => {
                t!("add.adopt", count = self.add_flow.selected.len()).to_string()
            }
            None if has_scan => {
                t!("common.add_count", count = self.add_flow.selected.len()).to_string()
            }
            None if self.add_flow.kind == AddKind::Existing => t!("add.scan").to_string(),
            None => t!("add.detect").to_string(),
        };
        let close_button = kit::close_button(p, "close-add-modal", can_close)
            .when(can_close, |button| {
                button.on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx)))
            });
        let cancel = kit::button(
            p,
            "cancel-add",
            kit::ButtonKind::Ghost,
            kit::Size::Md,
            can_close,
        )
        .child(t!("common.cancel").to_string())
        .when(can_close, |button| {
            button.on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_dialog(cx)),
            )
        });
        let confirm = kit::button(
            p,
            "confirm-add",
            kit::ButtonKind::Primary,
            kit::Size::Md,
            ready || pending,
        )
        .when(pending, |button| {
            button
                .cursor_default()
                .child(self.shell.spinner_on_accent.clone())
        })
        .child(action_label)
        .when(ready, |button| {
            button.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if has_scan {
                        this.import_scanned_skills(cx);
                    } else {
                        this.scan_add_source(cx);
                    }
                }),
            )
        });
        let panel = kit::modal_panel(p)
            .child(kit::modal_header(
                t!("common.add_skill").to_string(),
                close_button,
            ))
            .child(
                kit::modal_body()
                    .id("add-modal-content")
                    .max_h(px(520.))
                    .overflow_y_scroll()
                    .child(form)
                    .when(self.add_flow.task == Some(AddTask::Scanning), |body| {
                        body.child(self.scan_progress(window, cx))
                    }),
            )
            .child(
                kit::modal_footer(p)
                    .child(cancel)
                    .when(self.add_flow.kind != AddKind::Local || has_scan, |footer| {
                        footer.child(confirm)
                    }),
            );
        let _ = window;
        panel
    }
}
