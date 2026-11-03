//! Agent management views backed by the plugin registry and management API.
use super::*;
use crate::agent_plugins::{
    AgentRegistry, PluginOrigin,
    manage::{self, CandidateStatus, FetchedSource, SourceRecord},
};
use gpui_component::{Disableable, switch::Switch};

pub(super) struct AgentsSettings {
    input: Entity<InputState>,
    local: bool,
    file: Option<PathBuf>,
    pending: bool,
    generation: u64,
    fetched: Option<Arc<FetchedSource>>,
    selected: HashSet<String>,
    error: Option<String>,
    source: Option<(String, SourceRecord)>,
    drop_target: Option<(String, bool)>,
    busy: HashSet<String>,
    updates: HashSet<String>,
    checked_updates: bool,
    refreshed: HashSet<String>,
}

impl AgentsSettings {
    pub(super) fn new(window: &mut Window, cx: &mut Context<KitterApp>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.agents_settings.error = None;
                this.notify_dialog(cx);
            }
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.submit_agent_add(cx);
            }
        })
        .detach();
        Self {
            input,
            local: false,
            file: None,
            pending: false,
            generation: 0,
            fetched: None,
            selected: HashSet::new(),
            error: None,
            source: None,
            drop_target: None,
            busy: HashSet::new(),
            updates: HashSet::new(),
            checked_updates: false,
            refreshed: HashSet::new(),
        }
    }

    pub(super) fn cancel_add(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
        self.fetched = None;
        self.selected.clear();
    }
}

#[derive(Clone, Copy)]
enum AgentAction {
    Source,
    Reload,
    Remove,
}

#[derive(Clone)]
struct AgentDrag {
    id: String,
    name: String,
}
impl Render for AgentDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        kit::drag_preview(cx, self.name.clone())
    }
}

impl KitterApp {
    fn agents_changed(&mut self, cx: &mut Context<Self>) {
        let registry = AgentRegistry::global();
        if self
            .projects_view
            .selected_project_agent
            .as_ref()
            .is_some_and(|id| !registry.agents().iter().any(|agent| agent.id == *id))
        {
            self.projects_view.selected_project_agent = None;
        }
        components::clear_agent_images();
        self.refresh(cx);
        self.notify_dialog(cx);
    }

    pub(super) fn check_agent_updates(&mut self, cx: &mut Context<Self>) {
        if self.agents_settings.checked_updates {
            return;
        }
        self.agents_settings.checked_updates = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { manage::check_updates() })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(ids) = result {
                    this.agents_settings.updates.extend(
                        ids.into_iter()
                            .filter(|id| !this.agents_settings.refreshed.contains(id)),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn run_agent_change(
        &mut self,
        id: String,
        refreshed: bool,
        operation: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if !self.agents_settings.busy.insert(id.clone()) {
            return;
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { operation() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.agents_settings.busy.remove(&id);
                match result {
                    Ok(()) => {
                        if refreshed {
                            this.agents_settings.updates.remove(&id);
                            this.agents_settings.refreshed.insert(id.clone());
                        }
                        this.agents_changed(cx);
                    }
                    Err(error) => this.show_notice(messages::error_message(&error), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn agent_action(&mut self, id: String, action: AgentAction, cx: &mut Context<Self>) {
        match action {
            AgentAction::Source => {
                self.agents_settings.source = manage::settings()
                    .sources
                    .get(&id)
                    .cloned()
                    .map(|record| (id, record));
                if self.agents_settings.source.is_some() {
                    self.open_dialog(DialogKind::AgentSource, cx);
                }
            }
            AgentAction::Reload => self.run_agent_change(
                id.clone(),
                true,
                move || manage::refresh(&id).map(|_| ()),
                cx,
            ),
            AgentAction::Remove => {
                self.run_agent_change(id.clone(), true, move || manage::remove(&id), cx)
            }
        }
    }

    pub(super) fn agent_settings(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let registry = AgentRegistry::global();
        let settings = manage::settings();
        let state = &self.agents_settings;
        let mut card = kit::card(p);
        // Build presentation rows on every render; registry remains the only list state.
        let mut rows = Vec::new();
        for agent in registry.all_agents().iter() {
            rows.push((
                agent.id.to_string(),
                agent.name.clone(),
                agent.origin.clone(),
                kit::agent_avatar(p, agent),
                None,
            ));
        }
        let mut listed = rows.iter().map(|row| row.0.clone()).collect::<HashSet<_>>();
        for diagnostic in registry.diagnostics().iter() {
            let Some(id) = &diagnostic.id else {
                continue;
            };
            if !listed.insert(id.clone()) {
                continue;
            }
            rows.push((
                id.clone(),
                id.clone(),
                diagnostic.origin.clone(),
                kit::agent_icon_avatar(p, &crate::agent_plugins::AgentIcon::Monogram, id),
                Some(diagnostic.message.clone()),
            ));
        }
        for (index, (id, name, origin, avatar, error)) in rows.into_iter().enumerate() {
            let builtin = matches!(origin, PluginOrigin::Builtin);
            let managed = settings.sources.contains_key(&id);
            let path = match origin {
                PluginOrigin::User(path) | PluginOrigin::Override(path) => Some(path),
                _ => None,
            };
            let app = cx.entity().downgrade();
            let menu_id = id.clone();
            let menu = (!builtin || managed).then(|| {
                Popover::new(ElementId::Name(format!("agent-menu-{id}").into()))
                    .appearance(false)
                    .anchor(Anchor::TopRight)
                    .trigger(kit::icon_trigger(
                        p,
                        ElementId::Name(format!("agent-more-{id}").into()),
                        "icons/ellipsis.svg",
                        t!("agent_manage.more").to_string(),
                    ))
                    .content(move |_, _, menu_cx| {
                        let mut menu = kit::menu(p);
                        if managed {
                            for (action, icon, label) in [
                                (
                                    AgentAction::Source,
                                    "icons/github.svg",
                                    t!("agent_manage.source").to_string(),
                                ),
                                (
                                    AgentAction::Reload,
                                    "icons/rotate-cw.svg",
                                    t!("agent_manage.reload").to_string(),
                                ),
                                (
                                    AgentAction::Remove,
                                    "icons/trash.svg",
                                    t!("common.remove").to_string(),
                                ),
                            ] {
                                if matches!(action, AgentAction::Remove) {
                                    menu = menu.child(kit::menu_separator(p));
                                }
                                let app = app.clone();
                                let id = menu_id.clone();
                                menu = menu.child(
                                    kit::menu_action(
                                        p,
                                        ElementId::Name(
                                            format!("agent-action-{id}-{}", action as u8).into(),
                                        ),
                                        icon,
                                        label,
                                    )
                                    .on_click(
                                        menu_cx.listener(move |_, _, _, cx| {
                                            let _ = app.update(cx, |this, cx| {
                                                this.agent_action(id.clone(), action, cx)
                                            });
                                            cx.emit(DismissEvent);
                                        }),
                                    ),
                                );
                            }
                        } else if let Some(path) = path.clone() {
                            menu = menu.child(
                                kit::menu_action(
                                    p,
                                    "agent-reveal",
                                    "icons/folder.svg",
                                    t!("common.show_in_folder").to_string(),
                                )
                                .on_click(menu_cx.listener(
                                    move |_, _, _, cx| {
                                        cx.reveal_path(&path);
                                        cx.emit(DismissEvent);
                                    },
                                )),
                            );
                        }
                        menu
                    })
            });
            let key = id.clone();
            let update_id = id.clone();
            let toggle_id = id.clone();
            let drag_id = id.clone();
            let drop_id = id.clone();
            let app = cx.entity().downgrade();
            let enabled = registry.is_enabled(&id);
            // Failed plugins are not in the registry, so they cannot be toggled or reordered.
            let loaded = error.is_none();
            let group: SharedString = format!("agent-row-{id}").into();
            let drop_position = state
                .drop_target
                .as_ref()
                .filter(|(target, _)| target == &id && cx.has_active_drag())
                .map(|(_, after)| *after);
            let handle = div()
                .id(ElementId::Name(format!("agent-drag-{id}").into()))
                .w(px(16.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .cursor(CursorStyle::OpenHand)
                .opacity(0.)
                .group_hover(group.clone(), |handle| handle.opacity(1.))
                .tab_index(0)
                .focus_visible(|handle| handle.opacity(1.))
                .child(kit::grip(p));
            let mut controls = div().flex().items_center().gap(px(8.));
            if state.busy.contains(&id) {
                controls = controls.child(self.shell.spinner_accent.clone());
            } else if state.updates.contains(&id) {
                controls = controls.child(
                    kit::icon_button(
                        p,
                        ElementId::Name(format!("agent-update-{id}").into()),
                        "icons/rotate-cw.svg",
                        kit::IconKind::Ghost,
                        kit::Size::Sm,
                    )
                    .tooltip(|window, cx| {
                        Tooltip::new(t!("skills.update").to_string()).build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.agent_action(update_id.clone(), AgentAction::Reload, cx)
                    })),
                );
            }
            let switch = loaded.then(|| {
                Switch::new(ElementId::Name(format!("agent-toggle-{id}").into()))
                    .small()
                    .checked(enabled)
                    .disabled(state.busy.contains(&id))
                    .color(p.primary)
                    .tooltip(
                        t!(if enabled {
                            "agent_manage.disable"
                        } else {
                            "agent_manage.enable"
                        })
                        .to_string(),
                    )
                    .on_click(move |enabled, _, cx| {
                        let id = toggle_id.clone();
                        let enabled = *enabled;
                        let _ = app.update(cx, |this, cx| {
                            this.run_agent_change(
                                id.clone(),
                                false,
                                move || manage::set_enabled(&id, enabled),
                                cx,
                            )
                        });
                    })
            });
            let row = kit::card_row(p, index == 0)
                .id(ElementId::Name(format!("agent-row-{id}").into()))
                .relative()
                .group(group);
            let row = if loaded {
                row.on_drag(
                    AgentDrag {
                        id: id.clone(),
                        name: name.clone(),
                    },
                    |drag, _, _, cx| cx.new(|_| drag.clone()),
                )
                .child(handle)
            } else {
                row.child(div().w(px(16.)).flex_none())
            };
            let row = row
                .child(avatar)
                .child(div().min_w_0().flex_1().truncate().child(name))
                .when(builtin, |row| {
                    row.child(kit::badge(p).child(t!("projects.built_in").to_string()))
                })
                .when_some(error, |row, message| {
                    row.child(
                        div()
                            .id(ElementId::Name(format!("agent-error-{key}").into()))
                            .child(Self::icon(
                                "icons/triangle-alert.svg",
                                icon_sm(),
                                p.secondary,
                            ))
                            .tooltip(move |window, cx| {
                                Tooltip::new(message.clone()).build(window, cx)
                            }),
                    )
                })
                .child(controls)
                .when_some(switch, |row, switch| row.child(switch))
                .when_some(menu, |row, menu| row.child(menu))
                .when(builtin && !managed, |row| {
                    row.child(div().w(px(h_sm())).flex_none())
                });
            if !loaded {
                card = card.child(row);
                continue;
            }
            card = card.child(
                row.on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<AgentDrag>, _, cx| {
                        let next = (event.bounds.contains(&event.event.position)
                            && event.drag(cx).id != drag_id)
                            .then(|| {
                                (
                                    drag_id.clone(),
                                    event.event.position.y >= event.bounds.center().y,
                                )
                            });
                        let state = &mut this.agents_settings;
                        if (next.is_some()
                            || state
                                .drop_target
                                .as_ref()
                                .is_some_and(|(target, _)| target == &drag_id))
                            && state.drop_target != next
                        {
                            state.drop_target = next;
                            cx.notify();
                        }
                    },
                ))
                .on_drop(cx.listener(move |this, drag: &AgentDrag, _, cx| {
                    let (target, after) = this
                        .agents_settings
                        .drop_target
                        .take()
                        .filter(|(target, _)| target == &drop_id)
                        .unwrap_or((drop_id.clone(), false));
                    if drag.id != target {
                        let id = drag.id.clone();
                        this.run_agent_change(
                            id.clone(),
                            false,
                            move || manage::move_agent(&id, &target, after),
                            cx,
                        );
                    }
                    cx.notify();
                }))
                .when_some(drop_position, |row, after| {
                    let line = kit::drop_line(p);
                    row.child(if after { line.bottom_0() } else { line.top_0() })
                }),
            );
        }
        div()
            .mt(px(32.))
            .child(
                div()
                    .mb(px(8.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(kit::section_title(t!("settings.agents").to_string()).mb(px(0.)))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                kit::icon_button(
                                    p,
                                    "agent-directory",
                                    "icons/folder.svg",
                                    kit::IconKind::Secondary,
                                    kit::Size::Sm,
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new(t!("agent_manage.open_directory").to_string())
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        let path = crate::agent_plugins::plugin_dir();
                                        match std::fs::create_dir_all(&path) {
                                            Ok(()) => cx.reveal_path(&path),
                                            Err(error) => this.show_notice(
                                                t!(
                                                    "settings.plugin_directory_failed",
                                                    error = error
                                                )
                                                .to_string(),
                                                cx,
                                            ),
                                        }
                                    },
                                )),
                            )
                            .child(
                                kit::icon_button(
                                    p,
                                    "agent-add",
                                    "icons/plus.svg",
                                    kit::IconKind::Secondary,
                                    kit::Size::Sm,
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new(t!("agent_manage.add").to_string())
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        let state = &mut this.agents_settings;
                                        state.cancel_add();
                                        state.local = false;
                                        state.file = None;
                                        state.error = None;
                                        state.input.update(cx, |input, cx| {
                                            input.set_value("", window, cx);
                                            input.set_placeholder(
                                                t!("agent_manage.url_placeholder").to_string(),
                                                window,
                                                cx,
                                            );
                                        });
                                        this.open_dialog(DialogKind::AgentAdd, cx);
                                    },
                                )),
                            ),
                    ),
            )
            .child(card)
    }

    fn submit_agent_add(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.agents_settings;
        if state.pending
            || !self
                .shell
                .dialog_body
                .as_ref()
                .is_some_and(|body| matches!(body.read(cx).kind, DialogKind::AgentAdd))
        {
            return;
        }
        if let Some(fetched) = state.fetched.clone() {
            let ids = fetched
                .candidates
                .iter()
                .filter(|candidate| state.selected.contains(&candidate.id))
                .map(|candidate| candidate.id.clone())
                .collect::<Vec<_>>();
            if ids.is_empty() {
                return;
            }
            state.pending = true;
            state.error = None;
            let generation = state.generation;
            self.notify_dialog(cx);
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { manage::add(&fetched, &ids) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    // The write may finish after the sheet closes; always invalidate project caches.
                    if result.is_ok() {
                        this.agents_changed(cx);
                    }
                    if this.agents_settings.generation != generation {
                        return;
                    }
                    this.agents_settings.pending = false;
                    match result {
                        Ok(()) => this.close_dialog(cx),
                        Err(error) => {
                            this.agents_settings.error = Some(messages::error_message(&error));
                            this.notify_dialog(cx);
                        }
                    }
                });
            })
            .detach();
            return;
        }
        let value = if state.local {
            let Some(path) = &state.file else {
                return;
            };
            path.to_string_lossy().into_owned()
        } else {
            state.input.read(cx).value().trim().to_string()
        };
        let input = match manage::parse_source(&value) {
            Ok(input) => input,
            Err(error) => {
                state.error = Some(messages::error_message(&error));
                self.notify_dialog(cx);
                return;
            }
        };
        state.pending = true;
        state.error = None;
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        self.notify_dialog(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { manage::fetch(input) })
                .await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.agents_settings;
                if state.generation != generation {
                    return;
                }
                state.pending = false;
                match result {
                    Ok(fetched) => {
                        state.selected = fetched
                            .candidates
                            .iter()
                            .filter(|c| {
                                matches!(
                                    c.status,
                                    CandidateStatus::Ready | CandidateStatus::Override
                                )
                            })
                            .map(|c| c.id.clone())
                            .collect();
                        state.fetched = Some(Arc::new(fetched));
                    }
                    Err(error) => state.error = Some(messages::error_message(&error)),
                }
                this.notify_dialog(cx);
            });
        })
        .detach();
    }
    fn agent_choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(t!("common.choose").to_string().into()),
        });
        let generation = self.agents_settings.generation;
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let _ = this.update(cx, |this, cx| {
                    let state = &mut this.agents_settings;
                    if state.generation != generation {
                        return;
                    }
                    if let Some(path) = paths.into_iter().next() {
                        if path.is_dir() || path.extension().is_some_and(|ext| ext == "js") {
                            state.file = Some(path);
                            state.error = None;
                        } else {
                            state.error = Some(t!("agent_manage.invalid_file").to_string());
                        }
                    }
                    this.notify_dialog(cx);
                });
            }
        })
        .detach();
    }

    pub(super) fn agent_add_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let state = &self.agents_settings;
        let pending = state.pending;
        let choosing = state.fetched.is_some();
        let ready = !pending
            && if choosing {
                !state.selected.is_empty()
            } else if state.local {
                state.file.is_some()
            } else {
                !state.input.read(cx).value().trim().is_empty()
            };
        let mut tabs = kit::segmented(p).w_full();
        for (local, label) in [
            (false, t!("agent_manage.github").to_string()),
            (true, t!("agent_manage.local").to_string()),
        ] {
            tabs = tabs.child(
                kit::segment(
                    p,
                    if local {
                        "plugin-tab-local"
                    } else {
                        "plugin-tab-github"
                    },
                    label,
                    state.local == local,
                )
                .flex_1()
                .justify_center()
                .when(!pending, |button| {
                    button.on_click(cx.listener(move |this, _, _, cx| {
                        let state = &mut this.agents_settings;
                        state.local = local;
                        state.error = None;
                        this.notify_dialog(cx);
                    }))
                }),
            );
        }
        let mut form = kit::modal_body().flex().flex_col().gap(px(16.)).child(tabs);
        if choosing {
            let mut candidates = div()
                .id("plugin-candidates")
                .flex()
                .flex_col()
                .gap(px(2.))
                .max_h(px(240.))
                .overflow_y_scroll();
            for (index, candidate) in state
                .fetched
                .as_ref()
                .unwrap()
                .candidates
                .iter()
                .enumerate()
            {
                let selectable = matches!(
                    candidate.status,
                    CandidateStatus::Ready | CandidateStatus::Override
                );
                let selected = state.selected.contains(&candidate.id);
                let id = candidate.id.clone();
                let reason = match &candidate.status {
                    CandidateStatus::AlreadyAdded => {
                        Some(t!("agent_manage.already_added").to_string())
                    }
                    CandidateStatus::Invalid(error) => Some(error.clone()),
                    _ => None,
                };
                candidates = candidates.child(
                    div()
                        .id(ElementId::Name(format!("plugin-candidate-{index}").into()))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .px(px(10.))
                        .py(px(12.))
                        .rounded(px(r_md()))
                        .when(selectable && !pending, |row| {
                            row.cursor_pointer().hover_fill(p)
                        })
                        .child(
                            Checkbox::new(ElementId::Name(format!("plugin-check-{index}").into()))
                                .checked(selected)
                                .disabled(!selectable || pending),
                        )
                        .child(kit::agent_icon_avatar(p, &candidate.icon, &candidate.name))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .child(candidate.name.clone()),
                        )
                        .when_some(reason, |row, reason| {
                            row.child(
                                div()
                                    .id(ElementId::Name(
                                        format!("candidate-warning-{index}").into(),
                                    ))
                                    .child(Self::icon(
                                        "icons/triangle-alert.svg",
                                        icon_sm(),
                                        p.secondary,
                                    ))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(reason.clone()).build(window, cx)
                                    }),
                            )
                        })
                        .when(
                            matches!(candidate.status, CandidateStatus::Override),
                            |row| {
                                row.tooltip(|window, cx| {
                                    Tooltip::new(t!("agent_manage.override_hint").to_string())
                                        .build(window, cx)
                                })
                            },
                        )
                        .when(selectable && !pending, |row| {
                            row.on_click(cx.listener(move |this, _, _, cx| {
                                let state = &mut this.agents_settings;
                                if !state.selected.remove(&id) {
                                    state.selected.insert(id.clone());
                                }
                                this.notify_dialog(cx);
                            }))
                        }),
                );
            }
            form = kit::modal_body().child(candidates);
        } else if state.local {
            let label = state
                .file
                .as_ref()
                .map(|path| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_else(|| t!("agent_manage.choose_file").to_string());
            form = form.child(
                kit::button(
                    p,
                    "plugin-choose-file",
                    kit::ButtonKind::Secondary,
                    kit::Size::Md,
                    !pending,
                )
                .child(Self::icon("icons/folder.svg", icon_md(), p.secondary))
                .child(div().truncate().child(label))
                .when(!pending, |button| {
                    button.on_click(
                        cx.listener(|this, _, window, cx| this.agent_choose_file(window, cx)),
                    )
                }),
            );
        } else {
            form = form.child(kit::input(p, &state.input).disabled(pending));
        }
        if let Some(error) = &state.error {
            form = form.child(
                div()
                    .text_size(px(fs_small()))
                    .text_color(p.danger)
                    .child(error.clone()),
            );
        }
        kit::modal_panel(p)
            .child(kit::modal_header(
                t!("agent_manage.add").to_string(),
                kit::close_button(p, "plugin-close", true)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_agent_add(cx))),
            ))
            .child(form)
            .child(
                kit::modal_footer(p)
                    .child(
                        kit::button(
                            p,
                            "plugin-cancel",
                            kit::ButtonKind::Ghost,
                            kit::Size::Md,
                            true,
                        )
                        .child(t!("common.cancel").to_string())
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_agent_add(cx))),
                    )
                    .child(
                        kit::button(
                            p,
                            "plugin-confirm",
                            kit::ButtonKind::Primary,
                            kit::Size::Md,
                            ready || pending,
                        )
                        .when(pending, |button| {
                            button
                                .cursor_default()
                                .child(self.shell.spinner_on_accent.clone())
                        })
                        .child(
                            t!(if choosing {
                                "agent_manage.add_button"
                            } else {
                                "agent_manage.next"
                            })
                            .to_string(),
                        )
                        .when(ready, |button| {
                            button.on_click(cx.listener(|this, _, _, cx| this.submit_agent_add(cx)))
                        }),
                    ),
            )
    }

    fn cancel_agent_add(&mut self, cx: &mut Context<Self>) {
        self.close_dialog(cx);
    }

    pub(super) fn agent_source_modal(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let mut body = kit::modal_body().flex().flex_col().gap(px(16.));
        if let Some((id, record)) = &self.agents_settings.source {
            let registry = AgentRegistry::global();
            let agent = registry
                .all_agents()
                .iter()
                .find(|agent| agent.id.as_str() == id);
            let (avatar, name) = if let Some(agent) = agent {
                (kit::agent_avatar(p, agent), agent.name.clone())
            } else {
                (
                    kit::agent_icon_avatar(p, &crate::agent_plugins::AgentIcon::Monogram, id),
                    id.clone(),
                )
            };
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(avatar)
                    .child(name),
            );
            let (label, url, local) = match record {
                SourceRecord::Github { repo, .. } => (
                    repo.clone(),
                    Some(format!("https://github.com/{repo}")),
                    None,
                ),
                SourceRecord::Local { path } => (display_path(path), None, Some(path.clone())),
            };
            body = body.child(
                kit::button(
                    p,
                    "agent-source-link",
                    kit::ButtonKind::Quiet,
                    kit::Size::Sm,
                    true,
                )
                .justify_start()
                .w_full()
                .min_w_0()
                .child(div().min_w_0().flex_1().truncate().child(label))
                .child(Self::icon(
                    if local.is_some() {
                        "icons/folder-open.svg"
                    } else {
                        "icons/github.svg"
                    },
                    icon_sm(),
                    p.secondary,
                ))
                .on_click(cx.listener(move |_, _, _, cx| {
                    if let Some(path) = &local {
                        cx.reveal_path(path);
                    }
                    if let Some(url) = &url {
                        cx.open_url(url);
                    }
                })),
            );
            if let SourceRecord::Github {
                branch,
                commit,
                path,
                ..
            } = record
            {
                for (label, value) in [
                    (t!("agent_manage.branch").to_string(), branch.clone()),
                    (
                        t!("agent_manage.commit").to_string(),
                        commit.chars().take(7).collect(),
                    ),
                    (t!("agent_manage.path").to_string(), path.clone()),
                ] {
                    body = body.child(
                        div()
                            .flex()
                            .gap(px(16.))
                            .justify_between()
                            .text_size(px(fs_small()))
                            .child(div().flex_none().text_color(p.muted).child(label))
                            .child(div().min_w_0().truncate().font_family(mono()).child(value)),
                    );
                }
            }
        }
        kit::modal_panel(p)
            .child(kit::modal_header(
                t!("agent_manage.source").to_string(),
                kit::close_button(p, "agent-source-close", true)
                    .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
            ))
            .child(body)
            .child(
                kit::modal_footer(p).child(
                    kit::button(
                        p,
                        "agent-source-done",
                        kit::ButtonKind::Ghost,
                        kit::Size::Md,
                        true,
                    )
                    .child(t!("agent_manage.close").to_string())
                    .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                ),
            )
    }
}
