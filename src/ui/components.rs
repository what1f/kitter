use std::{collections::HashMap, sync::LazyLock};

use super::*;

type BrandImageCache = HashMap<(crate::agent_plugins::AgentId, bool), Arc<RenderImage>>;
static BRAND_IMAGES: LazyLock<Mutex<BrandImageCache>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// Rasterised coloured theme icons by file.
static COLOR_ICONS: LazyLock<Mutex<HashMap<PathBuf, Arc<RenderImage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// Coloured icons are rasterised once at this size, in device pixels, and
/// scaled down to where they are drawn.
const COLOR_ICON_PIXELS: i32 = 96;

/// A built-in icon, or the active theme's replacement for it. Coloured
/// replacements ignore the text colour.
pub(super) enum ThemeIcon {
    Mono(Svg),
    Color(Img),
}

impl IntoElement for ThemeIcon {
    type Element = AnyElement;

    fn into_element(self) -> AnyElement {
        match self {
            Self::Mono(icon) => icon.into_any_element(),
            Self::Color(icon) => icon.into_any_element(),
        }
    }
}

impl Styled for ThemeIcon {
    fn style(&mut self) -> &mut StyleRefinement {
        match self {
            Self::Mono(icon) => icon.style(),
            Self::Color(icon) => icon.style(),
        }
    }
}

impl InteractiveElement for ThemeIcon {
    fn interactivity(&mut self) -> &mut Interactivity {
        match self {
            Self::Mono(icon) => icon.interactivity(),
            Self::Color(icon) => icon.interactivity(),
        }
    }
}

/// Forget rasterised coloured icons, so an edited theme draws its new files.
pub(super) fn clear_color_icons() {
    COLOR_ICONS.lock().unwrap().clear();
}

fn color_icon(file: PathBuf) -> Img {
    img(ImageSource::Custom(Arc::new(move |_, cx| {
        if let Some(image) = COLOR_ICONS.lock().unwrap().get(&file) {
            return Some(Ok(image.clone()));
        }
        let image = std::fs::read(&file)
            .map_err(|error| ImageCacheError::Io(Arc::new(error)))
            .and_then(|bytes| {
                let renderer = cx.svg_renderer();
                let svg = renderer.parse_svg(&bytes)?;
                let pixels = DevicePixels(COLOR_ICON_PIXELS);
                Ok(renderer.render_parsed(&svg, SvgSize::Size(size(pixels, pixels)))?)
            });
        if let Ok(image) = &image {
            COLOR_ICONS
                .lock()
                .unwrap()
                .insert(file.clone(), image.clone());
        }
        Some(image)
    })))
}

impl KitterApp {
    /// Asset path for a built-in icon such as `icons/folder.svg`: the active
    /// theme's replacement when it has one, else Kitter's drawing.
    pub(super) fn icon_path(path: &'static str) -> SharedString {
        let name = path
            .strip_prefix("icons/")
            .and_then(|name| name.strip_suffix(".svg"))
            .unwrap_or(path);
        if let Some(icon) = theme::icon_override(name) {
            return icon;
        }
        // Keep existing call-site names while routing common icons to Lucide variants.
        match path {
            "icons/sparkle.svg" => IconName::Asterisk.path(),
            "icons/check.svg" => IconName::Check.path(),
            "icons/chevron-down.svg" => IconName::ChevronDown.path(),
            "icons/chevron-right.svg" => IconName::ChevronRight.path(),
            "icons/chevron-up.svg" => IconName::ChevronUp.path(),
            "icons/ellipsis.svg" => IconName::Ellipsis.path(),
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

    pub(super) fn icon(path: &'static str, size: f32, color: impl Into<Hsla>) -> ThemeIcon {
        let name = path
            .strip_prefix("icons/")
            .and_then(|name| name.strip_suffix(".svg"))
            .unwrap_or(path);
        match theme::color_icon(name) {
            Some(file) => ThemeIcon::Color(color_icon(file).size(px(size))),
            None => ThemeIcon::Mono(Self::mono_icon(path, size, color)),
        }
    }

    /// [`Self::icon`] that always takes the text colour, for icons that
    /// are transformed, such as spinners.
    pub(super) fn mono_icon(path: &'static str, size: f32, color: impl Into<Hsla>) -> Svg {
        svg()
            .path(Self::icon_path(path))
            .size(px(size))
            .text_color(color.into())
    }

    pub(super) fn danger_menu_item(label: impl Into<SharedString>, danger: Rgba) -> PopupMenuItem {
        let label = label.into();
        PopupMenuItem::element(move |_, _| div().text_color(danger).child(label.clone())).icon(
            Icon::default()
                .path(Self::icon_path("icons/trash.svg"))
                .text_color(danger),
        )
    }

    pub(super) fn brand_icon(
        agent: &crate::agent_plugins::AgentDef,
        size: f32,
        dark: bool,
    ) -> AnyElement {
        use crate::agent_plugins::AgentIcon;
        let (path, inline, size) = match &agent.icon {
            AgentIcon::Builtin(path) => (Some(*path), None, Self::brand_icon_size(path, size)),
            AgentIcon::Inline {
                light,
                dark: dark_svg,
            } => (
                None,
                Some(
                    if dark {
                        dark_svg.as_ref().unwrap_or(light)
                    } else {
                        light
                    }
                    .clone(),
                ),
                size,
            ),
            AgentIcon::Monogram => {
                return div()
                    .child(agent.name.chars().next().unwrap_or_default().to_string())
                    .into_any_element();
            }
        };
        let key = (agent.id.clone(), dark);
        img(ImageSource::Custom(Arc::new(move |_, cx| {
            let mut cache = BRAND_IMAGES.lock().unwrap();
            if let Some(image) = cache.get(&key) {
                return Some(Ok(image.clone()));
            }
            let bytes = if let Some(bytes) = &inline {
                std::borrow::Cow::Borrowed(bytes.as_ref())
            } else {
                let path = path?;
                let dark_path = format!("{}.dark.svg", path.trim_end_matches(".svg"));
                dark.then(|| cx.asset_source().load(&dark_path).ok().flatten())
                    .flatten()
                    .or_else(|| cx.asset_source().load(path).ok().flatten())?
            };
            let image = cx
                .svg_renderer()
                .render_single_frame(bytes.as_ref(), 2.0)
                .map_err(ImageCacheError::from);
            if let Ok(image) = &image {
                cache.insert(key.clone(), image.clone());
            }
            Some(image)
        })))
        .size(px(size))
        .flex_none()
        .into_any_element()
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
        kit::badge(p)
            .child(Self::icon("icons/hand.svg", icon_xs(), p.muted))
            .child(t!("common.manual").to_string())
    }

    pub(super) fn managed_skill_badge(&self) -> Div {
        let p = self.palette();
        kit::badge(p)
            .child(Self::icon("icons/crown.svg", icon_xs(), p.muted))
            .child(t!("common.managed").to_string())
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
                t!("projects.refresh_context").to_string(),
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
                let is_global_only = crate::agent_plugins::AgentRegistry::global()
                    .agents()
                    .iter()
                    .find(|agent| agent.id.as_str() == estimate.agent.as_str())
                    .is_some_and(|agent| agent.global_only);
                is_global_context || !is_global_only
            })
            .collect::<Vec<_>>();
        ordered_estimates.sort_by_key(|estimate| {
            crate::agent_plugins::AgentRegistry::global()
                .agents()
                .iter()
                .position(|agent| agent.id.as_str() == estimate.agent.as_str())
                .unwrap_or(usize::MAX)
        });
        let can_expand = ordered_estimates.len() > 8;
        let mut cards = div().w_full().flex().flex_wrap();
        for estimate in ordered_estimates {
            let agent = estimate.agent.clone();
            let selected = self.projects_view.selected_project_agent == Some(agent.clone());
            let automatic_count = estimate
                .model_visible_count
                .saturating_sub(estimate.name_only_count);
            let Some(definition) =
                crate::agent_plugins::AgentRegistry::global().agent(estimate.agent.as_str())
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
            let automatic = t!("context.automatic", count = automatic_count).to_string();
            let detail = {
                let mut value =
                    t!("context.manual", count = estimate.manual_only_count).to_string();
                if estimate.conditional_count > 0 {
                    value.push_str(
                        &t!("context.conditional", count = estimate.conditional_count).to_string(),
                    );
                }
                if estimate.name_only_count > 0 {
                    value.push_str(
                        &t!("context.name_only", count = estimate.name_only_count).to_string(),
                    );
                }
                value
            };
            cards = cards.child(
                div()
                    .id(ElementId::Name(
                        format!("project-agent-filter-{}", agent.as_str()).into(),
                    ))
                    .w(px(176.))
                    .h(px(90.))
                    .flex_none()
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(r_md()))
                    .cursor_pointer()
                    .selected_fill(p, selected)
                    .hover_fill(p)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(fs_caption()))
                            .text_color(p.secondary)
                            .child(
                                div()
                                    .size(px(24.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(p.text)
                                    .child(Self::brand_icon(definition, 18., p.dark)),
                            )
                            .child(definition.name.as_str()),
                    )
                    .child(
                        div()
                            .mt(px(7.))
                            .font_family(mono())
                            .text_size(px(fs_emph()))
                            .font_weight(weight_semibold())
                            .text_color(severity)
                            .child(format!("≈ {} tokens", estimate.estimated_tokens)),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .items_center()
                            .text_size(px(fs_caption()))
                            .child(div().text_color(severity).child(automatic))
                            .child(div().text_color(p.muted).child(detail)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.projects_view.selected_project_agent =
                            (this.projects_view.selected_project_agent != Some(agent.clone()))
                                .then_some(agent.clone());
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
            let arrow = kit::icon_button(
                p,
                "toggle-project-agents",
                if expanded {
                    "icons/chevron-up.svg"
                } else {
                    "icons/chevron-down.svg"
                },
                kit::IconKind::Ghost,
                kit::Size::Sm,
            )
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
        let panel = kit::card(p)
            .relative()
            .flex_none()
            .child(cards)
            .children(expand_control)
            .child(refresh_button.absolute().top(px(7.)).right(px(7.)));
        div().w_full().flex_none().child(panel)
    }

    pub(super) fn project_skills_tabs(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let mut tabs = kit::tab_bar(p);
        for (tab, label) in [
            (ProjectSkillsTab::Skills, t!("common.skills").to_string()),
            (ProjectSkillsTab::Plugins, t!("common.plugins").to_string()),
            (ProjectSkillsTab::Presets, t!("presets.title").to_string()),
        ] {
            let selected = self.projects_view.project_skills_tab == tab;
            tabs = tabs.child(
                kit::tab(
                    p,
                    ElementId::Name(
                        format!(
                            "project-skills-tab-{}",
                            match tab {
                                ProjectSkillsTab::Skills => "skills",
                                ProjectSkillsTab::Plugins => "plugins",
                                ProjectSkillsTab::Presets => "presets",
                            }
                        )
                        .into(),
                    ),
                    selected,
                )
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.projects_view.project_skills_tab = tab;
                    if tab != ProjectSkillsTab::Skills {
                        this.projects_view.batch_project = None;
                        this.projects_view.batch_selected.clear();
                        this.projects_view.batch_filter = None;
                    }
                    cx.notify();
                })),
            );
        }
        tabs
    }

    pub(super) fn effective_plugins_list(
        &self,
        groups: &[EffectivePluginGroup],
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let list = div()
            .id("project-plugins-scroll")
            .flex_1()
            .min_h_0()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(28.))
            .overflow_y_scroll();
        if groups.is_empty() {
            return list.child(
                kit::empty_state(
                    p,
                    "icons/package.svg",
                    t!("projects.plugins_empty").to_string(),
                )
                .mt(px(40.)),
            );
        }
        let mut card = kit::card(p);
        let last = groups.len() - 1;
        for (index, group) in groups.iter().enumerate() {
            let expanded = self
                .projects_view
                .expanded_project_plugins
                .contains(&group.key);
            let toggle_key = group.key.clone();
            let agent = crate::agent_plugins::AgentRegistry::global()
                .agents()
                .iter()
                .find(|agent| agent.id.as_str() == group.agent.as_str());
            let agent_name = agent
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| group.agent.to_string());

            card = card.child(
                kit::card_row(p, index == 0)
                    .id(ElementId::Name(format!("plugin-{}", group.key).into()))
                    .h(px(56.))
                    .when(index == last && !expanded, |row| {
                        row.rounded_b(px(r_card_inner()))
                    })
                    .cursor_pointer()
                    .hover_fill(p)
                    .child(
                        agent
                            .map(|agent| kit::agent_avatar(p, agent))
                            .unwrap_or_else(|| kit::avatar(p, "icons/package.svg")),
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
                                    .child(group.display_name.clone()),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(fs_small()))
                                    .text_color(p.muted)
                                    .child(agent_name),
                            ),
                    )
                    .child(Self::icon(
                        if expanded {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        },
                        icon_sm(),
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
                for skill in &group.skills {
                    card = card.child(
                        kit::card_row(p, false)
                            .min_h(px(row_1() + 4.))
                            .pl(px(16. + avatar_size() + 12.))
                            .gap(px(8.))
                            .child(Self::icon("icons/package.svg", icon_sm(), p.muted))
                            .child(
                                div()
                                    .min_w_0()
                                    .font_family(mono())
                                    .text_size(px(fs_small()))
                                    .truncate()
                                    .child(skill.clone()),
                            ),
                    );
                }
            }
        }
        list.child(card)
    }

    pub(super) fn icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        kit::icon_button(
            self.palette(),
            id,
            path,
            kit::IconKind::Ghost,
            kit::Size::Sm,
        )
    }

    /// Bordered icon button that sits next to an input or select.
    pub(super) fn bordered_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        kit::icon_button(
            self.palette(),
            id,
            path,
            kit::IconKind::Secondary,
            kit::Size::Md,
        )
    }

    pub(super) fn danger_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        kit::icon_button(
            self.palette(),
            id,
            path,
            kit::IconKind::Danger,
            kit::Size::Sm,
        )
    }

    pub(super) fn labeled_icon_button(
        &self,
        id: impl Into<ElementId>,
        path: &'static str,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label: SharedString = label.into();
        self.icon_button(id, path, cx)
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
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
                   label: String,
                   icon_path: &'static str,
                   page: Page,
                   current: Page| {
            kit::list_row(p, id, page == current)
                .text_size(px(fs_emph()))
                .text_color(p.sidebar_text)
                .child(Self::icon(icon_path, icon_md(), p.sidebar_text))
                .child(label)
        };
        let sidebar = kit::sidebar_panel(p)
            .px(px(8.))
            .pb(px(8.))
            .pt(px(if cfg!(target_os = "macos") { 0. } else { 8. }));
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
                            t!("common.skills").to_string(),
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
                            t!("common.projects").to_string(),
                            "icons/folder.svg",
                            Page::Projects,
                            self.shell.page,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shell.page = Page::Projects;
                            cx.notify();
                        })),
                    )
                    .child(
                        nav(
                            "nav-presets",
                            t!("presets.title").to_string(),
                            "icons/layers.svg",
                            Page::Presets,
                            self.shell.page,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shell.page = Page::Presets;
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
                            t!("common.settings").to_string(),
                            "icons/settings.svg",
                            Page::Settings,
                            self.shell.page,
                        )
                        .w_full()
                        .when(self.app_update_available(), |row| {
                            row.child(div().flex_1()).child(
                                kit::count(p, "●")
                                    .flex_none()
                                    .text_size(px(8.))
                                    .text_color(p.accent),
                            )
                        })
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
        title: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Div {
        self.panel_header_content(drag_id, kit::pane_title(title), cx)
    }

    pub(super) fn panel_header_content(
        &self,
        drag_id: &'static str,
        title: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let header = kit::pane_header(p);
        header.child(
            self.window_drag_region(
                div()
                    .id(drag_id)
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(title),
                cx,
            ),
        )
    }

    pub(super) fn skills_manage_control(&self, cx: &mut Context<Self>) -> Popover {
        let p = self.palette();
        let app = cx.entity().downgrade();
        let labels = [
            t!("updates.update_all").to_string(),
            t!("common.manage_tags").to_string(),
            t!("common.manage_groups").to_string(),
        ];
        Popover::new("skills-manage-menu")
            .appearance(false)
            .anchor(Anchor::TopLeft)
            .trigger(kit::icon_trigger(
                p,
                "skills-manage-trigger",
                "icons/ellipsis.svg",
                t!("skills.more").to_string(),
            ))
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p);
                for (index, label) in labels.clone().into_iter().enumerate() {
                    let item_app = app.clone();
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("skills-manage-item-{index}").into()),
                            label,
                            false,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, _, cx| {
                                let _ = item_app.update(cx, |this, cx| match index {
                                    0 => this.request_skill_updates(SkillUpdateRequest::All, cx),
                                    1 => this.open_tag_dialog(TagScope::Skills, cx),
                                    _ => this.open_group_dialog(cx),
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            })
    }
}

/// Reloaded definitions may contain different artwork for an existing Agent id.
pub(super) fn clear_agent_images() {
    BRAND_IMAGES.lock().unwrap().clear();
}

impl KitterApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn action_dialog(
        &self,
        key: &'static str,
        title: String,
        body: impl IntoElement,
        confirm: String,
        danger: bool,
        enabled: bool,
        cancel: String,
        on_cancel: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        on_confirm: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        kit::modal_panel(p)
            .child(kit::modal_header(
                title,
                kit::close_button(p, ElementId::Name(format!("close-{key}").into()), true)
                    .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
            ))
            .child(kit::modal_body().child(body))
            .child(
                kit::modal_footer(p)
                    .child(
                        kit::button(
                            p,
                            ElementId::Name(format!("cancel-{key}").into()),
                            kit::ButtonKind::Ghost,
                            kit::Size::Md,
                            true,
                        )
                        .child(cancel)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| on_cancel(this, cx)),
                        ),
                    )
                    .child(
                        kit::button(
                            p,
                            ElementId::Name(format!("confirm-{key}").into()),
                            if danger {
                                kit::ButtonKind::Danger
                            } else {
                                kit::ButtonKind::Primary
                            },
                            kit::Size::Md,
                            enabled,
                        )
                        .child(confirm)
                        .when(enabled, |button| {
                            button.on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| on_confirm(this, cx)),
                            )
                        }),
                    ),
            )
    }
}
