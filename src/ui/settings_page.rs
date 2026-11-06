use super::*;

impl KitterApp {
    pub(super) fn settings_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let language_label = match self.model.library.config.language {
            Language::System => t!("settings.system").to_string(),
            Language::ZhCn => t!("settings.simplified_chinese").to_string(),
            Language::En => t!("settings.english").to_string(),
        };
        let app = cx.entity().downgrade();
        let selected_language = self.model.library.config.language;
        let language_control = Popover::new("language-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(kit::select_trigger(p, "language-select", language_label).w(px(180.)))
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(180.));
                for (index, (language, label)) in [
                    (Language::System, t!("settings.system").to_string()),
                    (
                        Language::ZhCn,
                        t!("settings.simplified_chinese").to_string(),
                    ),
                    (Language::En, t!("settings.english").to_string()),
                ]
                .into_iter()
                .enumerate()
                {
                    let active = selected_language == language;
                    let app = app.clone();
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("language-option-{index}").into()),
                            label,
                            active,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, window, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.set_language(language, window, cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            });
        let preferences_group = kit::card(p).child(
            kit::setting_row(
                p,
                true,
                t!("settings.language").to_string(),
                t!("settings.language_hint").to_string(),
            )
            .child(language_control),
        );
        let style = self.model.library.config.theme_style.clone();
        let style_options = theme_file::list_themes();
        let style_label = style_options
            .iter()
            .find(|option| option.id == style)
            .or_else(|| style_options.first())
            .map(|option| option.name.clone())
            .unwrap_or_default();
        let style_app = cx.entity().downgrade();
        let style_control = Popover::new("style-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(kit::select_trigger(p, "style-select", style_label).w(px(180.)))
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(200.));
                for (index, option) in style_options.iter().enumerate() {
                    let app = style_app.clone();
                    let id = option.id.clone();
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("style-option-{index}").into()),
                            option.name.clone(),
                            style == option.id,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, window, cx| {
                                let id = id.clone();
                                let _ = app.update(cx, |this, cx| {
                                    this.model.library.config.theme_style = id;
                                    this.apply_appearance(window, cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu.child(kit::menu_separator(p)).child(
                    kit::menu_action(
                        p,
                        "style-open-folder",
                        "icons/folder.svg",
                        t!("settings.style_open_folder").to_string(),
                    )
                    .on_click(popover_cx.listener(|_, _, _, cx| {
                        let dir = theme_file::themes_dir();
                        let _ = std::fs::create_dir_all(&dir);
                        cx.open_with_system(&dir);
                        cx.emit(DismissEvent);
                    })),
                )
            });
        let appearance_group = kit::card(p)
            .child(
                kit::setting_row(
                    p,
                    true,
                    t!("settings.style").to_string(),
                    t!("settings.style_hint").to_string(),
                )
                .child(style_control),
            )
            .child(
                kit::setting_row(
                    p,
                    false,
                    t!("settings.theme").to_string(),
                    t!("settings.appearance_hint").to_string(),
                )
                .child(
                    kit::segmented(p)
                        .child(self.small_choice(
                            "theme-system",
                            t!("settings.system").to_string(),
                            self.model.library.config.theme == Theme::System,
                            cx,
                            |this| this.model.library.config.theme = Theme::System,
                        ))
                        .child(self.small_choice(
                            "theme-light",
                            t!("settings.light").to_string(),
                            self.model.library.config.theme == Theme::Light,
                            cx,
                            |this| this.model.library.config.theme = Theme::Light,
                        ))
                        .child(self.small_choice(
                            "theme-dark",
                            t!("settings.dark").to_string(),
                            self.model.library.config.theme == Theme::Dark,
                            cx,
                            |this| this.model.library.config.theme = Theme::Dark,
                        )),
                ),
            );
        let library_group = kit::card(p).child(
            kit::setting_row(
                p,
                true,
                t!("settings.data_location").to_string(),
                t!("settings.data_location_hint").to_string(),
            )
            .child(
                div()
                    .w(px(320.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .font_family(mono())
                            .text_size(px(fs_small()))
                            .text_color(p.secondary)
                            .truncate()
                            .child(self.selectable_text(
                                "settings-data-path",
                                400,
                                display_path(self.model.library.data_dir()),
                                window,
                                cx,
                            )),
                    )
                    .child(
                        self.bordered_icon_button("browse-data-location", "icons/folder.svg", cx)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.browse_data_location(window, cx)
                                }),
                            ),
                    ),
            ),
        );

        self.check_agent_updates(cx);

        div()
            .relative()
            .flex_1()
            .h_full()
            .flex()
            .child(
                // The scroll area spans the whole pane so card shadows are
                // only cut at the window edge, never at a box of their own.
                div()
                    .id("settings-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(60.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(SETTINGS_CONTENT_MAX_WIDTH))
                            .mx_auto()
                            .child(kit::section_title(t!("settings.preferences").to_string()))
                            .child(preferences_group)
                            .child(
                                kit::section_title(t!("settings.appearance").to_string())
                                    .mt(px(32.)),
                            )
                            .child(appearance_group)
                            .child(kit::section_title(t!("settings.data").to_string()).mt(px(32.)))
                            .child(library_group)
                            .child(self.agent_settings(cx))
                            .child(kit::section_title(t!("settings.about").to_string()).mt(px(32.)))
                            .child(self.about_group(cx)),
                    ),
            )
            .child(self.window_drag_strip("settings-window-drag", 20., cx))
    }

    pub(super) fn data_location_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let Some(prompt) = &self.shell.data_location else {
            return div();
        };
        let existing = prompt.kind == crate::data_location::DataFolder::Existing;
        let (title, confirm) = match (existing, prompt.moving) {
            (true, _) => (t!("settings.data_use_title"), t!("settings.data_use")),
            (false, false) => (t!("settings.data_move_title"), t!("settings.data_move")),
            (false, true) => (t!("settings.data_move_title"), t!("settings.data_moving")),
        };
        let body = div()
            .child(
                div()
                    .font_family(mono())
                    .text_size(px(fs_small()))
                    .text_color(p.secondary)
                    .child(display_path(&prompt.target)),
            )
            .when(existing, |body| {
                body.child(
                    div()
                        .mt(px(8.))
                        .text_size(px(fs_body()))
                        .line_height(relative(1.5))
                        .text_color(p.secondary)
                        .child(t!("settings.data_use_hint").to_string()),
                )
            });
        self.action_dialog(
            "data-location",
            title.to_string(),
            body,
            confirm.to_string(),
            false,
            !prompt.moving,
            t!("common.cancel").to_string(),
            |this, cx| this.close_dialog(cx),
            |this, cx| this.confirm_data_location(cx),
            cx,
        )
    }
}
