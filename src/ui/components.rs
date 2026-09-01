use super::*;

impl KitterApp {
    pub(super) fn icon_path(path: &'static str) -> SharedString {
        // Keep existing call-site names while routing common icons to Lucide variants.
        match path {
            "icons/sparkle.svg" => IconName::Asterisk.path(),
            "icons/check.svg" => IconName::Check.path(),
            "icons/chevron-down.svg" => IconName::ChevronDown.path(),
            "icons/chevron-right.svg" => IconName::ChevronRight.path(),
            "icons/chevron-up.svg" => IconName::ChevronUp.path(),
            "icons/file.svg" => IconName::File.path(),
            "icons/folder-open.svg" => IconName::FolderOpen.path(),
            "icons/folder.svg" => IconName::Folder.path(),
            "icons/plus.svg" => IconName::Plus.path(),
            "icons/search.svg" => IconName::Search.path(),
            "icons/settings.svg" => IconName::Settings.path(),
            "icons/sidebar.svg" => IconName::PanelLeft.path(),
            "icons/x.svg" => IconName::Close.path(),
            _ => path.into(),
        }
    }

    pub(super) fn icon(path: &'static str, size: f32, color: impl Into<Hsla>) -> Svg {
        svg()
            .path(Self::icon_path(path))
            .size(px(size))
            .text_color(color.into())
    }

    pub(super) fn danger_menu_item(label: impl Into<SharedString>, danger: Rgba) -> PopupMenuItem {
        let label = label.into();
        PopupMenuItem::element(move |_, _| div().text_color(danger).child(label.clone()))
            .icon(Icon::default().path("icons/trash.svg").text_color(danger))
    }

    pub(super) fn brand_icon(path: &'static str, size: f32, color: impl Into<Hsla>) -> AnyElement {
        let color = color.into();
        if Self::is_color_brand_icon(path) {
            Self::color_brand_icon(path, size).into_any_element()
        } else {
            svg()
                .path(path)
                .size(px(size))
                .flex_none()
                .text_color(color)
                .into_any_element()
        }
    }

    pub(super) fn is_color_brand_icon(path: &'static str) -> bool {
        matches!(
            path,
            "icons/provider-codex.svg"
                | "icons/provider-claude.svg"
                | "icons/provider-openclaw.svg"
                | "icons/provider-amp.svg"
                | "icons/provider-antigravity.svg"
                | "icons/provider-trae.svg"
        )
    }

    pub(super) fn color_brand_icon(path: &'static str, size: f32) -> Img {
        let cache = match path {
            "icons/provider-codex.svg" => &CODEX_COLOR_IMAGE,
            "icons/provider-claude.svg" => &CLAUDE_COLOR_IMAGE,
            "icons/provider-openclaw.svg" => &OPENCLAW_COLOR_IMAGE,
            "icons/provider-amp.svg" => &AMP_COLOR_IMAGE,
            "icons/provider-antigravity.svg" => &ANTIGRAVITY_COLOR_IMAGE,
            "icons/provider-trae.svg" => &TRAE_COLOR_IMAGE,
            _ => unreachable!("color_brand_icon called for a monochrome icon"),
        };
        img(ImageSource::Custom(Arc::new(move |_, cx| {
            if let Some(image) = cache.get() {
                return Some(Ok(image.clone()));
            }
            let bytes = cx.asset_source().load(path).ok().flatten()?;
            let image = cx
                .svg_renderer()
                .render_single_frame(bytes.as_ref(), 2.0)
                .map_err(ImageCacheError::from);
            if let Ok(image) = &image {
                let _ = cache.set(image.clone());
            }
            Some(image)
        })))
        .size(px(size))
        .flex_none()
    }

    pub(super) fn brand_icon_size(path: &'static str, base: f32) -> f32 {
        match path {
            // These marks use less of their 24x24 viewBox than Pi, so scale
            // them up to the same perceived size instead of the same box.
            "icons/provider-claude.svg" => base * 1.4,
            "icons/provider-codex.svg" => base * 1.25,
            "icons/provider-opencode.svg" | "icons/provider-grok.svg" => base * 1.1,
            _ => base,
        }
    }

    pub(super) fn manual_skill_badge(&self) -> Div {
        let p = self.palette();
        div()
            .h(px(20.))
            .px(px(7.))
            .rounded(px(7.5))
            .bg(p.raised)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .text_color(p.muted)
            .child(Self::icon("icons/hand.svg", 11., p.muted))
            .child(self.tr("手动", "Manual"))
    }

    pub(super) fn managed_skill_badge(&self) -> Div {
        let p = self.palette();
        div()
            .h(px(20.))
            .px(px(7.))
            .rounded(px(7.5))
            .bg(p.raised)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .text_color(p.warning)
            .child(Self::icon("icons/crown.svg", 11., p.warning))
            .child(self.tr("托管", "Managed"))
    }

    pub(super) fn context_estimate_panel(
        &self,
        path: &PathBuf,
        estimates: &[AgentContextEstimate],
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let refresh_path = path.clone();
        let refresh_button = self
            .labeled_icon_button(
                ElementId::Name(format!("refresh-context-{}", path.display()).into()),
                "icons/rotate-cw.svg",
                p.secondary,
                self.tr("刷新 token 扫描", "Refresh token scan"),
                cx,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.refresh_context_estimate(refresh_path.clone(), cx);
                }),
            );
        let is_global_context = dirs::home_dir().as_ref().is_some_and(|home| home == path);
        let mut ordered_estimates = estimates
            .iter()
            .filter(|estimate| {
                let is_global_only = crate::agents::AGENT_ICON_ORDER
                    .iter()
                    .find(|agent| agent.id == estimate.agent.id())
                    .is_some_and(|agent| agent.global_only);
                is_global_context || !is_global_only
            })
            .collect::<Vec<_>>();
        ordered_estimates.sort_by_key(|estimate| {
            crate::agents::AGENT_ICON_ORDER
                .iter()
                .position(|agent| agent.id == estimate.agent.id())
                .unwrap_or(usize::MAX)
        });
        let can_expand = ordered_estimates.len() > 8;
        let mut cards = div().w_full().flex().flex_wrap();
        for estimate in ordered_estimates {
            let agent = estimate.agent;
            let selected = self.projects_view.selected_project_agent == Some(agent);
            let automatic_count = estimate
                .model_visible_count
                .saturating_sub(estimate.name_only_count);
            let Some(icon) = crate::agents::AGENT_ICON_ORDER
                .iter()
                .find(|agent| agent.id == estimate.agent.id())
                .map(|agent| agent.icon_path)
            else {
                continue;
            };
            let severity = if estimate.estimated_tokens >= CONTEXT_TOKEN_DANGER_THRESHOLD
                || automatic_count >= CONTEXT_COUNT_DANGER_THRESHOLD
            {
                p.danger
            } else if estimate.estimated_tokens > CONTEXT_TOKEN_WARNING_THRESHOLD
                || automatic_count > CONTEXT_COUNT_WARNING_THRESHOLD
            {
                p.warning
            } else {
                p.success
            };
            let automatic = if self.uses_english() {
                format!("{} automatic", automatic_count)
            } else {
                format!("{} 自动", automatic_count)
            };
            let detail = if self.uses_english() {
                let mut value = format!(" · {} manual", estimate.manual_only_count);
                if estimate.conditional_count > 0 {
                    value.push_str(&format!(" · {} conditional", estimate.conditional_count));
                }
                if estimate.name_only_count > 0 {
                    value.push_str(&format!(" · {} name-only", estimate.name_only_count));
                }
                value
            } else {
                let mut value = format!(" · {} 手动", estimate.manual_only_count);
                if estimate.conditional_count > 0 {
                    value.push_str(&format!(" · {} 条件", estimate.conditional_count));
                }
                if estimate.name_only_count > 0 {
                    value.push_str(&format!(" · {} 仅名称", estimate.name_only_count));
                }
                value
            };
            cards = cards.child(
                div()
                    .id(ElementId::Name(
                        format!("project-agent-filter-{}", agent.id()).into(),
                    ))
                    .w(px(176.))
                    .h(px(90.))
                    .flex_none()
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(12.))
                    .cursor_pointer()
                    .bg(if selected {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |card| card.bg(p.hover))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(
                                div()
                                    .size(px(24.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Self::brand_icon(
                                        icon,
                                        Self::brand_icon_size(icon, 18.),
                                        p.text,
                                    )),
                            )
                            .child(estimate.agent.label()),
                    )
                    .child(
                        div()
                            .mt(px(7.))
                            .font_family(MONO)
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(severity)
                            .child(format!("≈ {} tokens", estimate.estimated_tokens)),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .items_center()
                            .text_size(px(11.))
                            .child(div().text_color(severity).child(automatic))
                            .child(div().text_color(p.muted).child(detail)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.projects_view.selected_project_agent =
                            (this.projects_view.selected_project_agent != Some(agent))
                                .then_some(agent);
                        cx.notify();
                    })),
            );
        }
        let cards = cards.pt(px(8.)).pl(px(8.)).pr(px(42.)).when(
            can_expand && !self.projects_view.project_agents_expanded,
            |cards| cards.max_h(px(188.)).overflow_hidden(),
        );
        let expand_control = can_expand.then(|| {
            let expanded = self.projects_view.project_agents_expanded;
            let arrow = div()
                .id("toggle-project-agents")
                .size(px(28.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |button| button.bg(p.hover).text_color(p.text))
                .child(Self::icon(
                    if expanded {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    13.,
                    p.muted,
                ))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.projects_view.project_agents_expanded =
                        !this.projects_view.project_agents_expanded;
                    cx.notify();
                }));
            div()
                .h(px(44.))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(arrow.mt(px(5.)))
        });
        let panel = div()
            .relative()
            .w_full()
            .flex_none()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(cards)
            .children(expand_control)
            .child(refresh_button.absolute().top(px(7.)).right(px(7.)));
        div().w_full().flex_none().child(panel)
    }

    pub(super) fn project_skills_tabs(
        &self,
        skill_count: usize,
        plugin_skill_count: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let mut tabs = div()
            .h(px(43.))
            .px(px(28.))
            .flex_none()
            .flex()
            .items_end()
            .gap(px(20.))
            .border_b_1()
            .border_color(p.border);
        for (tab, label, count) in [
            (
                ProjectSkillsTab::Skills,
                self.tr("技能", "Skills"),
                skill_count,
            ),
            (
                ProjectSkillsTab::Plugins,
                self.tr("插件", "Plugins"),
                plugin_skill_count,
            ),
        ] {
            let selected = self.projects_view.project_skills_tab == tab;
            tabs = tabs.child(
                div()
                    .id(ElementId::Name(
                        format!(
                            "project-skills-tab-{}",
                            if tab == ProjectSkillsTab::Skills {
                                "skills"
                            } else {
                                "plugins"
                            }
                        )
                        .into(),
                    ))
                    .h(px(37.))
                    .px(px(2.))
                    .border_b_1()
                    .border_color(if selected { p.accent } else { rgba(0x00000000) })
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if selected { p.text } else { p.muted })
                    .child(label)
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(p.muted)
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.projects_view.project_skills_tab = tab;
                        cx.notify();
                    })),
            );
        }
        tabs
    }

    pub(super) fn effective_plugins_list(
        &self,
        groups: Vec<EffectivePluginGroup>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let mut list = div()
            .id("project-plugins-scroll")
            .flex_1()
            .min_h_0()
            .px(px(28.))
            .pb(px(28.))
            .overflow_y_scroll();
        if groups.is_empty() {
            return list.child(
                div()
                    .mt(px(70.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_color(p.muted)
                    .child(Self::icon("icons/package.svg", 28., p.muted))
                    .child(div().mt(px(10.)).text_size(px(14.)).child(self.tr(
                        "没有检测到插件加载的技能",
                        "No plugin-provided Skills detected",
                    ))),
            );
        }
        for group in groups {
            let expanded = self
                .projects_view
                .expanded_project_plugins
                .contains(&group.key);
            let toggle_key = group.key.clone();
            let agent = crate::agents::AGENT_ICON_ORDER
                .iter()
                .find(|agent| agent.id == group.agent.id());
            let agent_name = agent
                .map(|agent| agent.name)
                .unwrap_or_else(|| group.agent.label());
            let agent_icon = agent
                .map(|agent| agent.icon_path)
                .unwrap_or("icons/package.svg");
            let count = if self.uses_english() {
                format!("{} Skills", group.skills.len())
            } else {
                format!("{} Skills", group.skills.len())
            };
            let mut plugin = div().border_b_1().border_color(p.border).child(
                div()
                    .id(ElementId::Name(format!("plugin-{}", group.key).into()))
                    .h(px(58.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .cursor_pointer()
                    .hover(move |row| row.bg(p.hover))
                    .child(
                        div()
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Self::brand_icon(
                                agent_icon,
                                Self::brand_icon_size(agent_icon, 19.),
                                p.text,
                            )),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(group.display_name.clone()),
                            )
                            .child(
                                div()
                                    .mt(px(3.))
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .child(agent_name),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(MONO)
                            .text_size(px(10.))
                            .text_color(p.muted)
                            .child(count),
                    )
                    .child(Self::icon(
                        if expanded {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        },
                        14.,
                        p.muted,
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this
                            .projects_view
                            .expanded_project_plugins
                            .remove(&toggle_key)
                        {
                            this.projects_view
                                .expanded_project_plugins
                                .insert(toggle_key.clone());
                        }
                        cx.notify();
                    })),
            );
            if expanded {
                for skill in group.skills {
                    plugin = plugin.child(
                        div()
                            .h(px(36.))
                            .ml(px(40.))
                            .px(px(8.))
                            .border_t_1()
                            .border_color(p.border)
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .child(Self::icon("icons/package.svg", 12., p.muted))
                            .child(
                                div()
                                    .min_w_0()
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .truncate()
                                    .child(skill),
                            ),
                    );
                }
            }
            list = list.child(plugin);
        }
        list
    }

    pub(super) fn icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        color: impl Into<Hsla>,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let color = color.into();
        div()
            .id(id)
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |button| button.bg(p.hover))
            .child(Self::icon(path, 16., color))
    }

    pub(super) fn bordered_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        color: impl Into<Hsla>,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        div()
            .id(id)
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |button| button.bg(p.hover))
            .child(Self::icon(path, 16., color.into()))
    }

    pub(super) fn danger_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        div()
            .id(id)
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .bg(p.danger_soft)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |button| button.opacity(0.8))
            .child(Self::icon(path, 16., p.danger))
    }

    pub(super) fn labeled_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        color: impl Into<Hsla>,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.icon_button(id, path, color, cx)
            .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
    }

    pub(super) fn primary_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        label: &'static str,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        div()
            .id(id)
            .size(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS_CONTROL))
            .bg(p.raised)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |button| button.bg(p.hover))
            .child(Self::icon(path, 16., p.secondary))
            .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
    }

    pub(super) fn window_drag_region(
        &self,
        region: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        region
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    crate::platform::titlebar_double_click(window);
                }
            })
            .on_mouse_down_out(cx.listener(|this, _, _, _| {
                this.shell.header_drag_armed = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.shell.header_drag_armed = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.shell.header_drag_armed = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.shell.header_drag_armed {
                    this.shell.header_drag_armed = false;
                    crate::platform::start_window_move(window);
                }
            }))
    }

    pub(super) fn window_drag_strip(
        &self,
        id: &'static str,
        height: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.window_drag_region(
            div()
                .id(id)
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .right(px(0.))
                .h(px(height)),
            cx,
        )
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let nav = |id: &'static str,
                   label: &'static str,
                   icon_path: &'static str,
                   page: Page,
                   current: Page| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .h(px(ROW_HEIGHT))
                .px(px(8.))
                .rounded(px(RADIUS_LIST_ROW))
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .bg(if page == current {
                    p.selected
                } else {
                    rgba(0x00000000)
                })
                .text_size(px(14.))
                .line_height(relative(1.5))
                .text_color(p.sidebar_text)
                .hover(move |s| s.bg(p.hover))
                .child(Self::icon(icon_path, 16., p.sidebar_text))
                .child(label)
        };
        let sidebar = div()
            .w_full()
            .min_w_0()
            .min_h_0()
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .px(px(8.))
            .pb(px(8.))
            .pt(px(if cfg!(target_os = "macos") { 0. } else { 8. }))
            .bg(p.sidebar);
        let sidebar = if cfg!(target_os = "macos") {
            sidebar.child(
                self.window_drag_region(
                    div()
                        .id("sidebar-window-drag")
                        .w_full()
                        .h(px(46.))
                        .flex_none(),
                    cx,
                ),
            )
        } else {
            sidebar
        };
        sidebar
            .child(div().h(px(8.)).flex_none())
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        nav(
                            "nav-skills",
                            self.tr("技能", "Skills"),
                            "icons/package.svg",
                            Page::Skills,
                            self.shell.page,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shell.page = Page::Skills;
                            cx.notify();
                        })),
                    )
                    .child(
                        nav(
                            "nav-projects",
                            self.tr("项目", "Projects"),
                            "icons/folder.svg",
                            Page::Projects,
                            self.shell.page,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shell.page = Page::Projects;
                            cx.notify();
                        })),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .w_full()
                    .h(px(46.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .child(
                        nav(
                            "nav-settings",
                            self.tr("设置", "Settings"),
                            "icons/settings.svg",
                            Page::Settings,
                            self.shell.page,
                        )
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shell.page = Page::Settings;
                            cx.notify();
                        })),
                    ),
            )
    }

    pub(super) fn panel_header(
        &self,
        drag_id: &'static str,
        title: &'static str,
        count: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let header = div()
            .h(px(52.))
            .flex_none()
            .px(px(12.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(p.border);
        header.child(
            self.window_drag_region(
                div()
                    .id(drag_id)
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(count.to_string()),
                    ),
                cx,
            ),
        )
    }

    pub(super) fn dropdown_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        font_size: f32,
    ) -> gpui_base::Button {
        let p = self.palette();
        let label = label.into();
        gpui_base::Button::new(id)
            .accessibility_label(label.clone())
            .h(px(CONTROL_HEIGHT))
            .flex_shrink_0()
            .px(px(8.))
            .rounded(px(RADIUS_CONTROL))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .text_color(p.text)
            .font_family(FONT_UI)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(4.))
            .hover(move |button| button.bg(p.hover))
            .active(move |button| button.bg(p.selected))
            .styles(|styles| {
                styles
                    .selected(|style| style.bg(p.selected))
                    .disabled(|style| style.bg(p.surface).text_color(p.muted))
            })
            // Button::label clips text to a 1em line box in gpui-component.
            // Own the label so descenders fit while long values still ellipsize.
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(font_size))
                    .line_height(relative(1.5))
                    .child(label),
            )
            .child(Self::icon("icons/chevron-down.svg", 12., p.secondary).flex_none())
    }

    pub(super) fn skills_manage_control(&self, cx: &mut Context<Self>) -> Popover {
        let p = self.palette();
        let app = cx.entity().downgrade();
        Popover::new("skills-manage-menu")
            .appearance(false)
            .anchor(Anchor::TopLeft)
            .trigger(
                Button::new("skills-manage-trigger")
                    .small()
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(rgba(0x00000000).into())
                            .foreground(p.secondary.into())
                            .hover(p.hover.into())
                            .active(p.selected.into()),
                    )
                    .h(px(CONTROL_HEIGHT))
                    .w(px(CONTROL_HEIGHT))
                    .rounded(px(RADIUS_CONTROL))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(18.))
                            .text_color(p.secondary)
                            .child("⋯"),
                    ),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = div()
                    .w(px(150.))
                    .p(px(4.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.elevated)
                    .shadow_lg();
                for (index, label) in [(0, "检查更新"), (1, "标签管理"), (2, "分组管理")]
                {
                    let item_app = app.clone();
                    menu = menu.child(
                        div()
                            .id(ElementId::Name(
                                format!("skills-manage-item-{index}").into(),
                            ))
                            .h(px(30.))
                            .px(px(9.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .hover(move |row| row.bg(p.hover))
                            .child(label)
                            .on_click(popover_cx.listener(move |_, _, _, cx| {
                                let _ = item_app.update(cx, |this, cx| match index {
                                    0 => this.check_all_updates(cx),
                                    1 => this.open_tag_dialog(TagScope::Skills, cx),
                                    _ => this.open_group_dialog(cx),
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                menu
            })
    }
}
