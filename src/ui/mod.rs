use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_base::TextSelectionHandle;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IconNamed as _, IndexPath, Root, Sizable as _,
    Theme as ComponentTheme, ThemeMode as ComponentThemeMode,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    popover::Popover,
    select::{SelectEvent, SelectItem, SelectState},
    tooltip::Tooltip,
};

use crate::{
    InstallTarget, ProjectSkill, ProjectSkillInstallation, SkillGroup, SkillLibrary, SkillSummary,
    adoption,
    config::{Language, Theme},
    effective_skills::{self, AgentContextEstimate, AgentKind},
    project, source,
    tags::{TagId, TagState, load_tag_states_from, save_tag_states_to},
};

mod add_actions;
mod add_flow;
mod adoption_list;
mod badges;
mod components;
mod effective_view;
mod flows;
mod install_flow;
mod layout;
mod motion;
mod organize_flows;
mod overlay_actions;
mod project_actions;
mod projects_page;
mod selectable_text;
mod selection_actions;
mod settings_actions;
mod settings_page;
mod skill_selection;
mod skills_page;
mod state;
mod tags_groups_actions;
mod theme;

use effective_view::{EffectivePluginGroup, EffectiveSkillRow, same_file};
use gpui_component::resizable::ResizableState;
use skill_selection::SkillSelection;
use state::*;
use theme::*;

static CODEX_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();
static CLAUDE_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();
static OPENCLAW_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();
static AMP_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();
static ANTIGRAVITY_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();
static TRAE_COLOR_IMAGE: OnceLock<Arc<RenderImage>> = OnceLock::new();

fn display_path(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.display().to_string();
    };
    if path == home {
        return "~".into();
    }
    path.strip_prefix(&home)
        .map(|relative| format!("~/{}", relative.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

fn display_effective_root(path: &Path, project: &Path) -> String {
    path.strip_prefix(project)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|| display_path(path))
}

fn project_tag_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unique_installation_paths(installations: &[crate::ProjectSkillInstallation]) -> Vec<PathBuf> {
    let mut keys = HashSet::new();
    installations
        .iter()
        .filter_map(|installation| {
            keys.insert(project::installation_key(&installation.path))
                .then(|| installation.path.clone())
        })
        .collect()
}

fn unique_external_sources(kinds: &[project::RemovalKind]) -> Vec<String> {
    let mut seen = HashSet::new();
    kinds
        .iter()
        .filter_map(|kind| match kind {
            project::RemovalKind::ExternalLink { source } if seen.insert(source.clone()) => {
                Some(display_path(source))
            }
            _ => None,
        })
        .collect()
}

fn effective_plugin_groups(
    estimates: &[AgentContextEstimate],
    selected_agent: Option<AgentKind>,
) -> Vec<EffectivePluginGroup> {
    effective_view::plugin_groups(estimates, selected_agent)
}

fn effective_skill_rows(
    estimates: &[AgentContextEstimate],
    project_skills: &[ProjectSkill],
    selected_agent: Option<AgentKind>,
) -> Vec<EffectiveSkillRow> {
    effective_view::skill_rows(estimates, project_skills, selected_agent)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Skills,
    Projects,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagScope {
    Skills,
    Projects,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Installs,
    Content,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectSkillsTab {
    Skills,
    Plugins,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddKind {
    Local,
    Npx,
    Claude,
    Existing,
}

#[derive(Clone, PartialEq, Eq)]
struct SourceChoice {
    kind: AddKind,
    label: SharedString,
}

impl SelectItem for SourceChoice {
    type Value = AddKind;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            div()
                .font_family(FONT_UI)
                .text_size(px(13.))
                .child(self.label.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .font_family(FONT_UI)
            .text_size(px(13.))
            .child(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.kind
    }
}

#[derive(Clone, Copy)]
enum DialogKind {
    Add,
    Install,
    Delete,
    Tags,
    AssignTags,
    Groups,
    DeleteGroup,
    MoveGroup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagEdit {
    CreateRoot,
    CreateChild(TagId),
    Rename(TagId),
}

#[derive(Clone, PartialEq, Eq)]
enum GroupEdit {
    Create,
    Rename(String),
}

#[derive(Clone)]
struct TagAssignmentTarget {
    scope: TagScope,
    keys: Vec<String>,
    label: String,
}

#[derive(Clone)]
struct SkillDrag {
    name: String,
}

impl Render for SkillDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(RADIUS_CONTROL))
            .bg(rgb(0xeeeeee))
            .font_family(MONO)
            .text_size(px(12.))
            .child(self.name.clone())
    }
}

#[derive(Clone)]
struct TagDrag {
    scope: TagScope,
    parent: Option<TagId>,
    tag_id: TagId,
    name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagDropPosition {
    Before,
    After,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TagDropTarget {
    scope: TagScope,
    tag_id: TagId,
    position: TagDropPosition,
}

impl Render for TagDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(RADIUS_CONTROL))
            .bg(rgb(0xeeeeee))
            .font_family(MONO)
            .text_size(px(12.))
            .child(format!("#{}", self.name))
    }
}

struct DialogBody {
    app: Entity<KitterApp>,
    kind: DialogKind,
}

struct PageBody {
    app: WeakEntity<KitterApp>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddTask {
    Scanning,
    Importing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectChoice {
    path: PathBuf,
}

impl SelectItem for ProjectChoice {
    type Value = PathBuf;

    fn title(&self) -> SharedString {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
            .into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        .font_family(FONT_UI)
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(self.title()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .font_family(MONO)
                        .text_size(px(12.))
                        .text_color(rgb(0x858590))
                        .truncate()
                        .child(display_path(&self.path)),
                )
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .min_w_0()
            .py(px(2.))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .font_family(FONT_UI)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.title()),
            )
            .child(
                div()
                    .mt(px(2.))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(rgb(0x858590))
                    .truncate()
                    .child(display_path(&self.path)),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.path
    }
}

#[derive(Clone)]
enum DeleteConfirmation {
    LibrarySkills {
        skills: Vec<(String, PathBuf)>,
    },
    ProjectSkill {
        project: PathBuf,
        skill: crate::ProjectSkill,
    },
}

#[derive(Clone)]
struct ContentSnapshot {
    skill: String,
    file: PathBuf,
    files: Vec<PathBuf>,
    content: SharedString,
}

fn skill_storage_name(skill: &SkillSummary) -> &str {
    if skill.record.storage_name.is_empty() {
        &skill.record.name
    } else {
        &skill.record.storage_name
    }
}

const CONTEXT_ESTIMATE_CACHE_TTL: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Clone)]
struct ContextEstimateCache {
    scanned_at: Instant,
    estimates: Vec<AgentContextEstimate>,
}

struct SpinnerView {
    color: Rgba,
    size: f32,
}

impl Render for SpinnerView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        motion::spin(KitterApp::icon(
            "icons/rotate-cw.svg",
            self.size,
            self.color,
        ))
    }
}

pub struct KitterApp {
    model: AppModel,
    shell: ShellState,
    skills_view: SkillsState,
    projects_view: ProjectsState,
    add_flow: AddFlowState,
    install_flow: InstallFlowState,
    delete_flow: DeleteFlowState,
    tags_flow: TagsFlowState,
    groups_flow: GroupsFlowState,
}

impl KitterApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_in(crate::config::app_data_dir(), window, cx)
    }

    fn new_in(data_dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = SkillLibrary::open_in(&data_dir)
            .unwrap_or_else(|error| panic!("无法打开 Kitter：{error:#}"));
        let system_dark = matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let component_mode = match library.config.theme {
            Theme::Light => ComponentThemeMode::Light,
            Theme::Dark => ComponentThemeMode::Dark,
            Theme::System if system_dark => ComponentThemeMode::Dark,
            Theme::System => ComponentThemeMode::Light,
        };
        ComponentTheme::change(component_mode, Some(window), cx);
        let skills = library.list().unwrap_or_default();
        let selected = skills
            .first()
            .map(|skill| skill_storage_name(skill).to_string());
        let english = match library.config.language {
            Language::En => true,
            Language::ZhCn => false,
            Language::System => Language::system() == Language::En,
        };
        let skill_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if english {
                "Search Skills"
            } else {
                "搜索技能"
            })
        });
        let project_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if english {
                "Search projects"
            } else {
                "搜索项目"
            })
        });
        let tag_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if english {
                "Tag name"
            } else {
                "标签名称"
            })
        });
        let group_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if english {
                "Group name"
            } else {
                "分组名称"
            })
        });
        let add_primary_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if english {
                "Paste a skills.sh/GitHub URL or npx skills add command"
            } else {
                "粘贴 skills.sh、GitHub 地址或 npx skills add 命令"
            })
        });
        let add_source_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    SourceChoice {
                        kind: AddKind::Npx,
                        label: "skills.sh / github".into(),
                    },
                    SourceChoice {
                        kind: AddKind::Claude,
                        label: if english {
                            "Claude plugin"
                        } else {
                            "Claude 插件"
                        }
                        .into(),
                    },
                    SourceChoice {
                        kind: AddKind::Local,
                        label: if english {
                            "Local folder"
                        } else {
                            "本地文件夹"
                        }
                        .into(),
                    },
                    SourceChoice {
                        kind: AddKind::Existing,
                        label: if english {
                            "Existing installations"
                        } else {
                            "已有安装"
                        }
                        .into(),
                    },
                ],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let shell_layout = cx.new(|_| ResizableState::default());
        let content_layout = cx.new(|_| ResizableState::default());
        // Panel bounds notify during prepaint too. Only invalidate the page
        // when widths actually change, not on every layout measurement.
        for state in [&shell_layout, &content_layout] {
            let mut previous = Vec::new();
            cx.observe(state, move |_, state, cx| {
                let sizes = state.read(cx).sizes();
                if *sizes != previous {
                    previous = sizes.clone();
                    cx.notify();
                }
            })
            .detach();
        }
        let project_choices = library
            .config
            .project_paths()
            .into_iter()
            .map(|path| ProjectChoice { path })
            .collect::<Vec<_>>();
        let project_select = cx.new(|cx| {
            SelectState::new(
                project_choices,
                (!library.config.project_paths().is_empty()).then(IndexPath::default),
                window,
                cx,
            )
        });
        let initial_palette = match component_mode {
            ComponentThemeMode::Dark => Palette::dark(),
            _ => Palette::light(),
        };
        let spinner_accent = cx.new(|_| SpinnerView {
            color: initial_palette.accent,
            size: 14.,
        });
        let spinner_on_accent = cx.new(|_| SpinnerView {
            color: initial_palette.on_accent,
            size: 12.,
        });
        let app = cx.entity();
        let weak_app = app.downgrade();
        let page_body = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            PageBody { app: weak_app }
        });
        let skill_page_body = page_body.clone();
        cx.subscribe_in(
            &skill_search,
            window,
            move |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    skill_page_body.update(cx, |_, cx| cx.notify());
                }
            },
        )
        .detach();
        let project_page_body = page_body.clone();
        cx.subscribe_in(
            &project_search,
            window,
            move |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    project_page_body.update(cx, |_, cx| cx.notify());
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &tag_name_input,
            window,
            |this, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.tags_flow.error = None;
                    this.notify_dialog(cx);
                }
                InputEvent::PressEnter { .. } => this.commit_tag_edit(cx),
                InputEvent::Focus | InputEvent::Blur => {}
            },
        )
        .detach();
        cx.subscribe_in(
            &group_name_input,
            window,
            |this, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.tags_flow.error = None;
                    this.notify_dialog(cx);
                }
                InputEvent::PressEnter { .. } => this.commit_group_edit(cx),
                InputEvent::Focus | InputEvent::Blur => {}
            },
        )
        .detach();
        cx.subscribe_in(
            &project_select,
            window,
            |this, _, event: &SelectEvent<Vec<ProjectChoice>>, _, cx| {
                let SelectEvent::Confirm(Some(path)) = event else {
                    return;
                };
                this.projects_view.open_project = Some(path.clone());
                this.notify_dialog(cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &add_primary_input,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.notify_dialog(cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &add_source_select,
            window,
            |this, _, event: &SelectEvent<Vec<SourceChoice>>, window, cx| {
                let SelectEvent::Confirm(Some(kind)) = event else {
                    return;
                };
                this.add_flow.kind = *kind;
                let english = this.uses_english();
                let placeholder = match this.add_flow.kind {
                    AddKind::Claude => {
                        if english {
                            "Plugin name or claude plugin install command"
                        } else {
                            "插件名称或 claude plugin install 命令"
                        }
                    }
                    AddKind::Npx => {
                        if english {
                            "Paste a skills.sh/GitHub URL or npx skills add command"
                        } else {
                            "粘贴 skills.sh、GitHub 地址或 npx skills add 命令"
                        }
                    }
                    AddKind::Local | AddKind::Existing => "",
                };
                this.add_flow.primary_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.set_placeholder(placeholder, window, cx);
                });
                this.add_flow.scan = None;
                this.add_flow.adoption_scan = None;
                this.add_flow.adoption_expanded.clear();
                this.add_flow.selected.clear();
                this.add_flow.error = None;
                this.add_flow.group_enabled = true;
                this.add_flow.group_name = None;
                this.notify_dialog(cx);
            },
        )
        .detach();
        let (tags, project_tags) = load_tag_states_from(&data_dir);
        Self {
            model: AppModel {
                library,
                skills,
                checking_updates: false,
                updating_skill: None,
            },
            shell: ShellState {
                page: Page::Skills,
                focus_handle: cx.focus_handle(),
                header_drag_armed: false,
                shell_layout,
                content_layout,
                spinner_accent,
                spinner_on_accent,
                notice: None,
                notice_generation: 0,
                system_dark,
                dialog_body: None,
                page_body,
            },
            skills_view: SkillsState {
                expanded_description: None,
                tab: DetailTab::Installs,
                selection: SkillSelection::new(selected),
                selected_file: PathBuf::from("SKILL.md"),
                skill_search,
                content_snapshot: RefCell::new(None),
                content_scroll: ScrollHandle::new(),
                selectable_text_handles: RefCell::new(BTreeMap::new()),
                collapsed_groups: HashSet::new(),
                collapsed_content_directories: HashSet::new(),
            },
            projects_view: ProjectsState {
                open_project: None,
                global_project_view: true,
                project_skills_tab: ProjectSkillsTab::Skills,
                selected_project_agent: None,
                project_agents_expanded: false,
                expanded_project_plugins: HashSet::new(),
                project_search,
                project_select,
                project_snapshots: RefCell::new(BTreeMap::new()),
                project_snapshot_tasks: RefCell::new(BTreeMap::new()),
                context_estimates: RefCell::new(BTreeMap::new()),
                context_estimate_tasks: RefCell::new(BTreeMap::new()),
                scan_generation: 0,
            },
            add_flow: AddFlowState {
                kind: AddKind::Npx,
                task: None,
                scan: None,
                adoption_scan: None,
                adoption_rows: Vec::new(),
                adoption_list: ListState::new(0, ListAlignment::Top, px(80.)),
                adoption_cancel: None,
                adoption_expanded: HashSet::new(),
                adoption_root: None,
                selected: HashSet::new(),
                error: None,
                group_enabled: true,
                group_name: None,
                primary_input: add_primary_input,
                source_select: add_source_select,
            },
            install_flow: InstallFlowState {
                modal: false,
                global: true,
                selected_targets: HashSet::new(),
            },
            delete_flow: DeleteFlowState {
                confirmation: None,
                selected: HashSet::new(),
                busy: false,
            },
            tags_flow: TagsFlowState {
                name_input: tag_name_input,
                skills: tags,
                projects: project_tags,
                selected_skill_filter: None,
                selected_project_filter: None,
                scope: TagScope::Skills,
                drop_target: None,
                edit: None,
                delete_pending: None,
                error: None,
                assignment_keys: Vec::new(),
                assignment_label: None,
                return_to_assignment: None,
            },
            groups_flow: GroupsFlowState {
                name_input: group_name_input,
                edit: None,
                delete_pending: None,
                delete_skills: false,
                move_skills: Vec::new(),
            },
        }
    }

    fn palette(&self) -> Palette {
        match self.model.library.config.theme {
            Theme::Light => Palette::light(),
            Theme::Dark => Palette::dark(),
            Theme::System if !self.shell.system_dark => Palette::light(),
            Theme::System => Palette::dark(),
        }
    }

    fn content_pane_width(&self, window: &Window, cx: &App) -> f32 {
        let sidebar = self
            .shell
            .shell_layout
            .read(cx)
            .sizes()
            .first()
            .copied()
            .unwrap_or(px(layout::SIDEBAR_WIDTH));
        f32::from(window.bounds().size.width) - f32::from(sidebar).clamp(160., 260.)
    }

    fn tr(&self, zh: &'static str, en: &'static str) -> &'static str {
        if self.uses_english() { en } else { zh }
    }

    fn selectable_text(
        &self,
        id: impl Into<String>,
        document_order: u64,
        text: impl Into<SharedString>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> selectable_text::SelectableText {
        let id = id.into();
        let text = text.into();
        let focus_handle = self.shell.focus_handle.clone();
        let selection = {
            let mut handles = self.skills_view.selectable_text_handles.borrow_mut();
            handles
                .entry(id)
                .or_insert_with(|| {
                    let selection = TextSelectionHandle::new(text.to_string(), cx);
                    selection.refresh_window_on_change(window, cx).detach();
                    selection.focus_with(
                        move |window, cx| {
                            focus_handle.focus(window, cx);
                        },
                        cx,
                    );
                    selection
                })
                .clone()
        };
        selection.set_fallback_copy_text(text.to_string(), cx);
        selectable_text::SelectableText::new(selection, document_order, text)
    }

    fn uses_english(&self) -> bool {
        match self.model.library.config.language {
            Language::En => true,
            Language::ZhCn => false,
            Language::System => Language::system() == Language::En,
        }
    }

    fn tags_for(&self, scope: TagScope) -> &TagState {
        match scope {
            TagScope::Skills => &self.tags_flow.skills,
            TagScope::Projects => &self.tags_flow.projects,
        }
    }

    fn tags_for_mut(&mut self, scope: TagScope) -> &mut TagState {
        match scope {
            TagScope::Skills => &mut self.tags_flow.skills,
            TagScope::Projects => &mut self.tags_flow.projects,
        }
    }

    fn persist_tags(&self) {
        let _ = save_tag_states_to(
            self.model.library.data_dir(),
            &self.tags_flow.skills,
            &self.tags_flow.projects,
        );
    }

    fn tag_filter_for(&self, scope: TagScope) -> Option<TagId> {
        match scope {
            TagScope::Skills => self.tags_flow.selected_skill_filter,
            TagScope::Projects => self.tags_flow.selected_project_filter,
        }
    }

    fn set_tag_filter(&mut self, scope: TagScope, filter: Option<TagId>) {
        match scope {
            TagScope::Skills => self.tags_flow.selected_skill_filter = filter,
            TagScope::Projects => self.tags_flow.selected_project_filter = filter,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.model.skills = self.model.library.list().unwrap_or_default();
        self.projects_view.scan_generation = self.projects_view.scan_generation.wrapping_add(1);
        self.projects_view.project_snapshots.borrow_mut().clear();
        self.projects_view
            .project_snapshot_tasks
            .borrow_mut()
            .clear();
        self.projects_view.context_estimates.borrow_mut().clear();
        self.projects_view
            .context_estimate_tasks
            .borrow_mut()
            .clear();
        *self.skills_view.content_snapshot.borrow_mut() = None;
        let order = self
            .model
            .skills
            .iter()
            .map(|skill| skill_storage_name(skill).to_string())
            .collect::<Vec<_>>();
        self.skills_view.selection.reconcile(&order);
        cx.notify();
    }

    fn sync_spinner_palette(&self, cx: &mut Context<Self>) {
        let p = self.palette();
        self.shell.spinner_accent.update(cx, |spinner, cx| {
            spinner.color = p.accent;
            cx.notify();
        });
        self.shell.spinner_on_accent.update(cx, |spinner, cx| {
            spinner.color = p.on_accent;
            cx.notify();
        });
    }

    fn project_snapshot(
        &self,
        path: &PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<Vec<ProjectSkill>> {
        if let Some(snapshot) = self.projects_view.project_snapshots.borrow().get(path) {
            return Some(snapshot.clone());
        }

        self.request_project_snapshots(std::slice::from_ref(path), cx);
        None
    }

    fn request_project_snapshots(&self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let generation = self.projects_view.scan_generation;
        let mut pending = Vec::new();
        {
            let snapshots = self.projects_view.project_snapshots.borrow();
            let tasks = self.projects_view.project_snapshot_tasks.borrow();
            for path in paths {
                if !snapshots.contains_key(path)
                    && !tasks
                        .get(path)
                        .is_some_and(|running| *running == generation)
                {
                    pending.push(path.clone());
                }
            }
        }
        if pending.is_empty() {
            return;
        }
        {
            let mut tasks = self.projects_view.project_snapshot_tasks.borrow_mut();
            for path in &pending {
                tasks.insert(path.clone(), generation);
            }
        }

        let library_dir = self.model.library.config.library_dir.clone();
        cx.spawn(async move |this, cx| {
            let snapshots = cx
                .background_executor()
                .spawn(async move {
                    pending
                        .into_iter()
                        .map(|path| {
                            let snapshot = project::list(&path, &library_dir).unwrap_or_default();
                            (path, snapshot)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let mut changed = false;
                for (path, snapshot) in snapshots {
                    let is_current = this.projects_view.scan_generation == generation
                        && this
                            .projects_view
                            .project_snapshot_tasks
                            .borrow()
                            .get(&path)
                            .is_some_and(|running| *running == generation);
                    if !is_current {
                        continue;
                    }
                    this.projects_view
                        .project_snapshot_tasks
                        .borrow_mut()
                        .remove(&path);
                    this.projects_view
                        .project_snapshots
                        .borrow_mut()
                        .insert(path, snapshot);
                    changed = true;
                }
                if changed {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn context_estimate_snapshot(
        &self,
        path: &PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<Vec<AgentContextEstimate>> {
        let cached = self
            .projects_view
            .context_estimates
            .borrow()
            .get(path)
            .cloned();
        if cached
            .as_ref()
            .is_some_and(|cache| cache.scanned_at.elapsed() < CONTEXT_ESTIMATE_CACHE_TTL)
        {
            return cached.map(|cache| cache.estimates);
        }

        let generation = self.projects_view.scan_generation;
        if !self
            .projects_view
            .context_estimate_tasks
            .borrow()
            .get(path)
            .is_some_and(|running| *running == generation)
        {
            self.projects_view
                .context_estimate_tasks
                .borrow_mut()
                .insert(path.clone(), generation);
            let path = path.clone();
            cx.spawn(async move |this, cx| {
                let worker_path = path.clone();
                let estimates = cx
                    .background_executor()
                    .spawn(async move { effective_skills::estimate_project(&worker_path) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let is_current = this.projects_view.scan_generation == generation
                        && this
                            .projects_view
                            .context_estimate_tasks
                            .borrow()
                            .get(&path)
                            .is_some_and(|running| *running == generation);
                    if !is_current {
                        return;
                    }
                    this.projects_view
                        .context_estimate_tasks
                        .borrow_mut()
                        .remove(&path);
                    this.projects_view.context_estimates.borrow_mut().insert(
                        path,
                        ContextEstimateCache {
                            scanned_at: Instant::now(),
                            estimates,
                        },
                    );
                    cx.notify();
                });
            })
            .detach();
        }

        cached.map(|cache| cache.estimates)
    }

    fn refresh_context_estimate(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.projects_view.scan_generation = self.projects_view.scan_generation.wrapping_add(1);
        self.projects_view
            .context_estimate_tasks
            .borrow_mut()
            .clear();
        self.projects_view
            .context_estimates
            .borrow_mut()
            .remove(&path);
        cx.notify();
    }

    fn content_snapshot(&self, skill: &SkillSummary) -> ContentSnapshot {
        let storage_name = skill_storage_name(skill);
        if let Some(snapshot) = self.skills_view.content_snapshot.borrow().as_ref()
            && snapshot.skill == storage_name
            && snapshot.file == self.skills_view.selected_file
        {
            return snapshot.clone();
        }
        let files = self
            .model
            .library
            .files_by_storage(storage_name)
            .unwrap_or_default();
        let content = self
            .model
            .library
            .read_file_by_storage(storage_name, &self.skills_view.selected_file)
            .unwrap_or_else(|error| error.to_string());
        let snapshot = ContentSnapshot {
            skill: storage_name.to_string(),
            file: self.skills_view.selected_file.clone(),
            files,
            content: content.into(),
        };
        *self.skills_view.content_snapshot.borrow_mut() = Some(snapshot.clone());
        snapshot
    }

    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        self.model.library.config.language = language;
        let english = self.uses_english();
        self.skills_view.skill_search.update(cx, |input, cx| {
            input.set_placeholder(
                if english {
                    "Search Skills"
                } else {
                    "搜索技能"
                },
                window,
                cx,
            )
        });
        self.projects_view.project_search.update(cx, |input, cx| {
            input.set_placeholder(
                if english {
                    "Search projects"
                } else {
                    "搜索项目"
                },
                window,
                cx,
            )
        });
        self.tags_flow.name_input.update(cx, |input, cx| {
            input.set_placeholder(if english { "Tag name" } else { "标签名称" }, window, cx)
        });
        self.add_flow.source_select.update(cx, |select, cx| {
            select.set_items(
                vec![
                    SourceChoice {
                        kind: AddKind::Npx,
                        label: "skills.sh / github".into(),
                    },
                    SourceChoice {
                        kind: AddKind::Claude,
                        label: if english {
                            "Claude plugin"
                        } else {
                            "Claude 插件"
                        }
                        .into(),
                    },
                    SourceChoice {
                        kind: AddKind::Local,
                        label: if english {
                            "Local folder"
                        } else {
                            "本地文件夹"
                        }
                        .into(),
                    },
                    SourceChoice {
                        kind: AddKind::Existing,
                        label: if english {
                            "Existing installations"
                        } else {
                            "已有安装"
                        }
                        .into(),
                    },
                ],
                window,
                cx,
            )
        });
        let _ = self.model.library.save();
        cx.notify();
    }

    fn sync_project_select(&self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = self
            .model
            .library
            .config
            .project_paths()
            .into_iter()
            .map(|path| ProjectChoice { path })
            .collect::<Vec<_>>();
        let selected = self
            .projects_view
            .open_project
            .as_ref()
            .and_then(|path| choices.iter().position(|choice| &choice.path == path))
            .or_else(|| (!choices.is_empty()).then_some(0));
        self.projects_view.project_select.update(cx, |select, cx| {
            select.set_items(choices, window, cx);
            select.set_selected_index(
                selected.map(|row| IndexPath::default().row(row)),
                window,
                cx,
            );
        });
    }
}

impl Render for DialogBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app.update(cx, |app, app_cx| match self.kind {
            DialogKind::Add => app.add_skill_modal(window, app_cx),
            DialogKind::Install => app.install_modal(window, app_cx),
            DialogKind::Delete => app.delete_confirmation_modal(window, app_cx),
            DialogKind::Tags => app.tag_management_modal(window, app_cx),
            DialogKind::AssignTags => app.tag_assignment_modal(window, app_cx),
            DialogKind::Groups => app.group_management_modal(window, app_cx),
            DialogKind::DeleteGroup => app.group_delete_modal(app_cx),
            DialogKind::MoveGroup => app.move_group_modal(window, app_cx),
        })
    }
}

impl Render for PageBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app
            .update(cx, |app, app_cx| match app.shell.page {
                Page::Skills => app.skills_page(window, app_cx),
                Page::Projects => app.projects_page(window, app_cx),
                Page::Settings => app.settings_page(window, app_cx),
            })
            .unwrap_or_else(|_| div())
    }
}

impl Render for KitterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let content = self.shell.page_body.clone();
        let mut root = div()
            .size_full()
            .relative()
            .track_focus(&self.shell.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("escape")
                    && this.skills_view.selection.is_multiple()
                {
                    this.finish_skill_selection_mode(cx);
                    cx.stop_propagation();
                }
            }))
            .font_family(FONT_UI)
            .text_color(p.text)
            .text_size(px(14.))
            .bg(p.window)
            .flex();
        let main = div()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .h_full()
            .bg(p.base)
            .flex()
            .flex_col();
        root = root.child(layout::shell(
            &self.shell.shell_layout,
            self.sidebar(cx),
            main.child(content),
        ));
        // Linux uses a borderless GPUI window, so it still needs a client-side
        // outline. Windows and macOS already provide their own window frame.
        if cfg!(target_os = "linux") {
            root = root.child(
                div()
                    .absolute()
                    .top(px(0.))
                    .right(px(0.))
                    .bottom(px(0.))
                    .left(px(0.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(p.window_border),
            );
        }
        if let Some(notice) = self.shell.notice.clone() {
            root = root.child(
                div()
                    .absolute()
                    .right(px(18.))
                    .bottom(px(18.))
                    .max_w(px(420.))
                    .px(px(13.))
                    .py(px(9.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.surface)
                    .shadow_md()
                    .text_size(px(13.))
                    .child(notice),
            );
        }
        if let Some(body) = self.shell.dialog_body.clone() {
            let width = match body.read(cx).kind {
                DialogKind::Add => px(520.),
                DialogKind::Install => px(600.),
                DialogKind::Delete => px(420.),
                DialogKind::Tags | DialogKind::AssignTags => px(420.),
                DialogKind::Groups => px(420.),
                DialogKind::DeleteGroup => px(460.),
                DialogKind::MoveGroup => px(420.),
            };
            root = root.child(
                div()
                    .absolute()
                    .top(px(0.))
                    .right(px(0.))
                    .bottom(px(0.))
                    .left(px(0.))
                    .bg(p.overlay)
                    .flex()
                    .items_center()
                    .justify_center()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .w(width)
                            .max_h(px(680.))
                            .rounded(px(RADIUS_MODAL))
                            .border_1()
                            .border_color(p.border)
                            .overflow_hidden()
                            .shadow_lg()
                            .child(body),
                    ),
            );
        }
        root
    }
}

#[cfg(test)]
mod window_tests {
    use super::{fit_initial_window_size, fit_minimum_window_size};
    use gpui::{px, size};

    #[test]
    fn initial_window_fits_inside_a_scaled_laptop_display() {
        let fitted = fit_initial_window_size(Some(size(px(1200.), px(720.))));

        assert_eq!(fitted.width, px(1020.));
        assert_eq!(fitted.height, px(576.));
        let minimum = fit_minimum_window_size(Some(size(px(1200.), px(720.))));
        assert_eq!(minimum.width, px(900.));
        assert_eq!(minimum.height, px(540.));
    }

    #[test]
    fn initial_window_keeps_its_designed_size_on_a_large_display() {
        let fitted = fit_initial_window_size(Some(size(px(2560.), px(1440.))));

        assert_eq!(fitted.width, px(1180.));
        assert_eq!(fitted.height, px(760.));
        let minimum = fit_minimum_window_size(Some(size(px(2560.), px(1440.))));
        assert_eq!(minimum.width, px(940.));
        assert_eq!(minimum.height, px(620.));
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod e2e_tests {
    use super::{AddKind, KitterApp, Page};
    use gpui::{AppContext, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point, px};
    use std::fs;

    fn init(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
    }

    #[test]
    fn injected_data_directory_is_the_only_library_source() {
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");

        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });

        cx.read_entity(&app, |app, _| {
            assert_eq!(app.model.library.data_dir(), data_dir);
            assert!(
                app.model
                    .skills
                    .iter()
                    .any(|skill| skill.record.name == "kitter")
            );
        });
        assert!(data_dir.join("registry.json").is_file());
        assert!(data_dir.join("skills/_kitter-builtin/SKILL.md").is_file());
    }

    #[test]
    fn sidebar_navigation_switches_the_rendered_page() {
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("kitter-data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        cx.refresh().unwrap();
        cx.run_until_parked();

        for (selector, expected) in [
            ("nav-projects", Page::Projects),
            ("nav-settings", Page::Settings),
            ("nav-skills", Page::Skills),
        ] {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} should be rendered"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            cx.read_entity(&app, |app, _| assert!(app.shell.page == expected));
        }
    }

    #[test]
    fn project_details_are_loaded_off_the_ui_thread() {
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let project = temp.path().join("fixture-project");
        let skill = project.join(".agents/skills/fixture-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: fixture-skill\ndescription: Fixture\n---\n",
        )
        .unwrap();

        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });

        app.update(cx, |app, cx| {
            assert!(app.project_snapshot(&project, cx).is_none());
            assert!(
                !app.projects_view
                    .project_snapshots
                    .borrow()
                    .contains_key(&project)
            );
            assert!(
                app.projects_view
                    .project_snapshot_tasks
                    .borrow()
                    .contains_key(&project)
            );
        });

        cx.run_until_parked();
        cx.read_entity(&app, |app, _| {
            let snapshots = app.projects_view.project_snapshots.borrow();
            let snapshot = snapshots
                .get(&project)
                .expect("background project scan should finish");
            assert_eq!(snapshot.len(), 1);
            assert_eq!(snapshot[0].name, "fixture-skill");
            assert!(
                !app.projects_view
                    .project_snapshot_tasks
                    .borrow()
                    .contains_key(&project)
            );
        });
    }

    #[test]
    fn large_project_renders_and_scrolls_with_virtual_rows() {
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let project = temp.path().join("fixture-project");
        for target in [".agents/skills", ".codex/skills", ".claude/skills"] {
            for index in 1..=180 {
                let name = format!("fixture-skill-{index:03}");
                let skill = project.join(target).join(&name);
                fs::create_dir_all(&skill).unwrap();
                fs::write(
                    skill.join("SKILL.md"),
                    format!("---\nname: {name}\ndescription: Performance fixture {index}\n---\n"),
                )
                .unwrap();
            }
        }

        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        app.update(cx, |app, cx| {
            app.shell.page = Page::Projects;
            app.projects_view.global_project_view = false;
            app.projects_view.open_project = Some(project.clone());
            cx.notify();
        });
        cx.refresh().unwrap();
        cx.run_until_parked();

        cx.read_entity(&app, |app, _| {
            assert_eq!(
                app.projects_view
                    .project_snapshots
                    .borrow()
                    .get(&project)
                    .expect("large project scan should finish")
                    .len(),
                180
            );
            assert!(
                app.projects_view
                    .context_estimates
                    .borrow()
                    .contains_key(&project)
            );
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("project-skill-fixture-skill-001").is_some());
        assert!(cx.debug_bounds("project-skill-fixture-skill-180").is_none());
        let scroll = cx
            .debug_bounds("project-skills-virtual-list")
            .expect("virtual project list should be rendered");
        // Avoid saturating GPUI's synthetic pixel delta at the exact maximum
        // offset; that test-only edge drops the virtual rows for the frame.
        cx.simulate_event(ScrollWheelEvent {
            position: scroll.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-11_000.))),
            ..Default::default()
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("project-skill-fixture-skill-180").is_some());
    }

    #[test]
    fn local_folder_picker_scans_inside_the_isolated_window() {
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let source = temp.path().join("fixture-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: fixture-skill\ndescription: Fixture\n---\n",
        )
        .unwrap();

        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        app.update_in(cx, |app, window, cx| {
            app.open_add_dialog(window, cx);
            app.set_add_kind(AddKind::Local, window, cx);
        });
        cx.refresh().unwrap();
        cx.run_until_parked();

        let choose = cx
            .debug_bounds("choose-local-skill")
            .expect("local folder control should be rendered");
        cx.simulate_click(choose.center(), Modifiers::none());
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response({
            let source = source.clone();
            move |options| {
                assert!(options.directories);
                assert!(!options.multiple);
                Some(vec![source])
            }
        });
        cx.run_until_parked();
        cx.read_entity(&app, |app, _| {
            let scan = app.add_flow.scan.as_ref().expect("scan should finish");
            assert_eq!(scan.skills().len(), 1);
            assert_eq!(scan.skills()[0].name, "fixture-skill");
            assert_eq!(app.model.library.data_dir(), data_dir);
        });

        let candidate = cx
            .debug_bounds("scan-skill-0")
            .expect("scanned Skill row should be rendered");
        cx.simulate_click(candidate.center(), Modifiers::none());
        cx.run_until_parked();
        let confirm = cx
            .debug_bounds("confirm-add")
            .expect("import control should be rendered");
        cx.simulate_click(confirm.center(), Modifiers::none());
        cx.run_until_parked();
        cx.read_entity(&app, |app, _| {
            assert!(
                app.model
                    .skills
                    .iter()
                    .any(|skill| skill.record.name == "fixture-skill")
            );
            assert!(app.shell.dialog_body.is_none());
        });
        assert!(data_dir.join("skills/fixture-skill/SKILL.md").is_file());
    }
}

fn fit_initial_window_size(display_size: Option<Size<Pixels>>) -> Size<Pixels> {
    let mut window_size = size(px(1180.), px(760.));
    if let Some(display_size) = display_size {
        // Keep the native title bar and resize frame comfortably inside the
        // visible display on high-DPI and smaller screens. GPUI centers against
        // the full monitor, so leave extra vertical room for taskbars and docks.
        window_size.width = window_size.width.min(display_size.width * 0.85);
        window_size.height = window_size.height.min(display_size.height * 0.80);
    }
    window_size
}

fn fit_minimum_window_size(display_size: Option<Size<Pixels>>) -> Size<Pixels> {
    let mut minimum_size = size(px(940.), px(620.));
    if let Some(display_size) = display_size {
        minimum_size.width = minimum_size.width.min(display_size.width * 0.75);
        minimum_size.height = minimum_size.height.min(display_size.height * 0.75);
    }
    minimum_size
}

fn initial_window_size(cx: &App) -> Size<Pixels> {
    fit_initial_window_size(cx.primary_display().map(|display| display.bounds().size))
}

fn minimum_window_size(cx: &App) -> Size<Pixels> {
    fit_minimum_window_size(cx.primary_display().map(|display| display.bounds().size))
}

pub fn run() {
    gpui_platform::application()
        .with_assets(crate::assets::Assets)
        .run(|cx: &mut App| {
            cx.set_app_identity("dev.kitter.app", "Kitter");
            gpui_component::init(cx);
            crate::assets::register_fonts(cx).expect("failed to register fonts");
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Kitter".into()),
                        appears_transparent: cfg!(target_os = "macos"),
                        traffic_light_position: cfg!(target_os = "macos")
                            .then(|| point(px(16.), px(17.))),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        initial_window_size(cx),
                        cx,
                    ))),
                    window_min_size: Some(minimum_window_size(cx)),
                    is_movable: true,
                    app_owns_titlebar_drag: cfg!(target_os = "macos"),
                    window_background: if cfg!(target_os = "macos") {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Opaque
                    },
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| {
                        let app = KitterApp::new(window, cx);
                        cx.observe_window_bounds(window, |_, window, cx| {
                            crate::platform::update_soft_window_shadow(window);
                            cx.notify();
                        })
                        .detach();
                        app
                    });
                    crate::platform::configure_window_material(window, cx.theme().is_dark());
                    // Kitter uses the platform window decorations. The component Root's
                    // client-side border adds a full-window mouse-move layer that is
                    // only needed by borderless Linux windows.
                    cx.new(|cx| {
                        Root::new(view, window, cx)
                            .bordered(false)
                            .bg(cx.theme().background)
                    })
                },
            )
            .expect("failed to open Kitter window");
        });
}
