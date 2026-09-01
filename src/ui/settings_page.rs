use super::*;

impl KitterApp {
    pub(super) fn settings_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let row = |title: &'static str, description: &'static str| {
            div()
                .min_h(px(60.))
                .px(px(16.))
                .py(px(12.))
                .flex()
                .items_center()
                .gap(px(24.))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(description),
                        ),
                )
        };
        let language_label = match self.model.library.config.language {
            Language::System => self.tr("跟随系统", "System"),
            Language::ZhCn => "简体中文",
            Language::En => "English",
        };
        let app = cx.entity().downgrade();
        let selected_language = self.model.library.config.language;
        let english = self.uses_english();
        let language_control = Popover::new("language-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(
                self.dropdown_button("language-select", language_label, 14.)
                    .w(px(180.)),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = div()
                    .w(px(180.))
                    .p(px(4.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.elevated)
                    .shadow_lg();
                for (index, (language, label)) in [
                    (
                        Language::System,
                        if english { "System" } else { "跟随系统" },
                    ),
                    (Language::ZhCn, "简体中文"),
                    (Language::En, "English"),
                ]
                .into_iter()
                .enumerate()
                {
                    let active = selected_language == language;
                    let app = app.clone();
                    menu = menu.child(
                        div()
                            .id(ElementId::Name(format!("language-option-{index}").into()))
                            .h(px(29.))
                            .px(px(8.))
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
                                    this.set_language(language, window, cx);
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                menu
            });
        let preferences_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    self.tr("语言", "Language"),
                    self.tr(
                        "选择 Kitter 界面的显示语言。",
                        "Choose the language used by Kitter.",
                    ),
                )
                .child(language_control),
            );
        let appearance_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    self.tr("主题", "Theme"),
                    self.tr(
                        "跟随系统，或固定使用浅色与深色外观。",
                        "Follow the system or use a fixed light or dark appearance.",
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(self.small_choice(
                            "theme-system",
                            self.tr("跟随系统", "System"),
                            self.model.library.config.theme == Theme::System,
                            cx,
                            |this| this.model.library.config.theme = Theme::System,
                        ))
                        .child(self.small_choice(
                            "theme-light",
                            self.tr("浅色", "Light"),
                            self.model.library.config.theme == Theme::Light,
                            cx,
                            |this| this.model.library.config.theme = Theme::Light,
                        ))
                        .child(self.small_choice(
                            "theme-dark",
                            self.tr("深色", "Dark"),
                            self.model.library.config.theme == Theme::Dark,
                            cx,
                            |this| this.model.library.config.theme = Theme::Dark,
                        )),
                ),
            );
        let library_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    self.tr("技能存放位置", "Skills location"),
                    self.tr(
                        "Kitter 用来保存所有技能的文件夹。",
                        "The folder where Kitter keeps all Skills.",
                    ),
                )
                .child(
                    div()
                        .w(px(320.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .font_family(MONO)
                                .text_size(px(12.))
                                .text_color(p.secondary)
                                .truncate()
                                .child(self.selectable_text(
                                    "settings-library-path",
                                    400,
                                    display_path(&self.model.library.config.library_dir),
                                    window,
                                    cx,
                                )),
                        )
                        .child(
                            self.bordered_icon_button(
                                "browse-library",
                                "icons/folder.svg",
                                p.text,
                                cx,
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.browse_library(window, cx)),
                            ),
                        ),
                ),
            );

        div()
            .relative()
            .flex_1()
            .h_full()
            .p(px(20.))
            .flex()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .max_w(px(SETTINGS_CONTENT_MAX_WIDTH))
                    .pb(px(40.))
                    .child(
                        div()
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.tr("偏好设置", "Preferences")),
                    )
                    .child(preferences_group)
                    .child(
                        div()
                            .mt(px(40.))
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.tr("外观", "Appearance")),
                    )
                    .child(appearance_group)
                    .child(
                        div()
                            .mt(px(40.))
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.tr("技能库", "Skill library")),
                    )
                    .child(library_group),
            )
            .child(self.window_drag_strip("settings-window-drag", 20., cx))
    }
}
