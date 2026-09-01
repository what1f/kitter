use super::*;

impl KitterApp {
    pub(super) fn agent_badges(
        &self,
        popover_id: String,
        targets: &[InstallTarget],
        include_global_only: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let agents = crate::agents::AGENT_ICON_ORDER
            .iter()
            .filter(|agent| {
                if agent.global_only {
                    include_global_only
                } else {
                    targets.iter().any(|target| agent.supports_target(*target))
                }
            })
            .map(|agent| (agent.name, agent.icon_path))
            .collect::<Vec<_>>();
        self.render_agent_badges(popover_id, agents, cx)
    }

    pub(super) fn effective_agent_badges(
        &self,
        popover_id: String,
        kinds: &[AgentKind],
        cx: &mut Context<Self>,
    ) -> Div {
        let agents = crate::agents::AGENT_ICON_ORDER
            .iter()
            .filter(|agent| kinds.iter().any(|kind| kind.id() == agent.id))
            .map(|agent| (agent.name, agent.icon_path))
            .collect::<Vec<_>>();
        self.render_agent_badges(popover_id, agents, cx)
    }

    pub(super) fn render_agent_badges(
        &self,
        popover_id: String,
        agents: Vec<(&'static str, &'static str)>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let hidden = agents.len().saturating_sub(5);
        let visible = agents.len().min(5);
        let badge_width = if visible == 0 {
            0.
        } else {
            25. + (visible.saturating_sub(1) as f32 * 20.)
        } + if hidden > 0 { 20. } else { 0. };
        let brand_color = p.text;
        let mut badges = div().flex().items_center();
        for (index, (_, icon)) in agents.iter().take(5).enumerate() {
            badges = badges.child(
                div()
                    .ml(if index == 0 { px(0.) } else { px(-5.) })
                    .size(px(25.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(0xffffff))
                    .bg(rgb(0xf5f5f7))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Self::brand_icon(
                        icon,
                        Self::brand_icon_size(icon, 17.),
                        p.text,
                    )),
            );
        }
        if hidden > 0 {
            badges = badges.child(
                Popover::new(ElementId::Name(format!("agent-menu-{popover_id}").into()))
                    .appearance(false)
                    .anchor(Anchor::TopRight)
                    .trigger(
                        Button::new(ElementId::Name(format!("agent-more-{popover_id}").into()))
                            .label(format!("+{hidden}"))
                            .xsmall()
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(p.raised.into())
                                    .foreground(p.muted.into())
                                    .hover(p.hover.into())
                                    .active(p.selected.into()),
                            )
                            .ml(px(-5.))
                            .size(px(25.))
                            .rounded(px(999.))
                            .font_family(MONO)
                            .text_size(px(8.)),
                    )
                    .content(move |_, _, _| {
                        let mut menu = div()
                            .id("agent-popover-scroll")
                            .w(px(176.))
                            .max_h(px(246.))
                            .p(px(4.))
                            .rounded(px(RADIUS_MENU))
                            .border_1()
                            .border_color(p.border_strong)
                            .bg(p.elevated)
                            .shadow_lg()
                            .overflow_y_scroll();
                        for (name, icon) in agents.clone() {
                            menu = menu.child(
                                div()
                                    .h(px(29.))
                                    .px(px(8.))
                                    .rounded(px(RADIUS_CONTROL))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .size(px(24.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Self::brand_icon(
                                                icon,
                                                Self::brand_icon_size(icon, 18.),
                                                brand_color,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(p.secondary)
                                            .child(name),
                                    ),
                            );
                        }
                        menu
                    }),
            );
        }
        div()
            .w(px(badge_width))
            .flex_none()
            .flex()
            .items_center()
            .child(badges)
    }
}
