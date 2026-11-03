use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_base::TextSelectionHandle;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IconNamed as _, IndexPath, Root, Sizable as _,
    ThemeMode as ComponentThemeMode,
    checkbox::Checkbox,
    input::{Editor, EditorState, Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    popover::Popover,
    select::{SelectEvent, SelectItem, SelectState},
    tooltip::Tooltip,
};

use crate::{
    InstallTarget, ProjectSkill, ProjectSkillInstallation, SkillGroup, SkillLibrary, SkillSummary,
    TriggerMode, adoption,
    config::{Language, Theme},
    effective_skills::{self, AgentContextEstimate, AgentId},
    project, source,
    tags::{TagId, TagState, load_tag_states_from, save_tag_states_to},
};

actions!(kitter, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

mod add_actions;
mod add_flow;
mod adoption_list;
mod agents_settings;
mod badges;
mod components;
mod effective_view;
mod file_tree;
mod flows;
mod install_flow;
mod kit;
mod layout;
mod messages;
mod motion;
mod organize_flows;
mod overlay_actions;
mod preset_actions;
mod presets_page;
mod project_actions;
mod projects_page;
mod selectable_text;
mod selection_actions;
mod settings_actions;
mod settings_page;
mod skill_selection;
mod skills_page;
mod state;
mod surface_art;
mod tags_groups_actions;
mod theme;
mod theme_file;

use effective_view::{
    EffectivePluginGroup, EffectiveSkillRow, ProjectRenderSnapshot, ProjectSnapshot,
};
use gpui_component::resizable::ResizableState;
use kit::{FillExt, HoverFillExt};
use skill_selection::SkillSelection;
use state::*;
use surface_art::SurfaceExt;
use theme::*;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Skills,
    Projects,
    Presets,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagScope {
    Skills,
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
    Presets,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PresetTab {
    Skills,
    AgentsMd,
    Projects,
}

#[derive(Clone, PartialEq, Eq)]
enum PresetEdit {
    Create,
    Rename(String),
    NewFile(String),
    RenameFile(String, PathBuf),
}

#[derive(Clone)]
enum PresetConfirmation {
    Delete(String),
    Uninstall(String, PathBuf),
    Overwrite(String, Vec<PathBuf>),
    DeleteFile(String, PathBuf),
    /// Project files that already exist with different content.
    Conflicts(Vec<PathBuf>),
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
                .font_family(font_ui())
                .text_size(px(fs_body()))
                .child(self.label.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .font_family(font_ui())
            .text_size(px(fs_body()))
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
    PresetName,
    PresetSkills,
    PresetConfirm,
    AgentAdd,
    AgentSource,
    DataLocation,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        kit::drag_preview(cx, self.name.clone())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GroupDragScope {
    Management,
    List,
}

#[derive(Clone)]
struct GroupDrag {
    scope: GroupDragScope,
    id: String,
    name: String,
}

impl Render for GroupDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        kit::drag_preview(cx, self.name.clone())
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        kit::drag_preview(cx, format!("#{}", self.name))
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
                        .font_family(font_ui())
                        .text_size(px(fs_body()))
                        .font_weight(weight_medium())
                        .child(self.title()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .font_family(mono())
                        .text_size(px(fs_small()))
                        .opacity(0.6)
                        .truncate()
                        .child(display_path(&self.path)),
                )
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .min_w_0()
            .py(px(2.))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .font_family(font_ui())
                    .text_size(px(fs_body()))
                    .font_weight(weight_medium())
                    .child(self.title()),
            )
            .child(
                div()
                    .mt(px(2.))
                    .font_family(mono())
                    .text_size(px(fs_small()))
                    .text_color(cx.theme().muted_foreground)
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
    ProjectSkills {
        project: PathBuf,
        skills: Vec<crate::ProjectSkill>,
        batch: bool,
    },
}

#[derive(Clone, PartialEq, Eq)]
enum ProjectSkillFilter {
    Group(String),
    Tag(TagId),
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
    snapshot: Arc<ProjectRenderSnapshot>,
}

struct SpinnerView {
    icon: &'static str,
    color: Rgba,
    size: f32,
}

impl Render for SpinnerView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        motion::spin(KitterApp::mono_icon(self.icon, self.size, self.color))
    }
}

pub struct KitterApp {
    model: AppModel,
    agents_settings: agents_settings::AgentsSettings,
    shell: ShellState,
    skills_view: SkillsState,
    projects_view: ProjectsState,
    add_flow: AddFlowState,
    install_flow: InstallFlowState,
    delete_flow: DeleteFlowState,
    tags_flow: TagsFlowState,
    groups_flow: GroupsFlowState,
    presets_view: PresetsState,
}

impl KitterApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut app = Self::new_in(crate::config::app_data_dir(), window, cx);
        app.start_background_update_check(cx);
        app
    }

    fn new_in(data_dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = SkillLibrary::open_in(&data_dir).unwrap_or_else(|error| {
            panic!(
                "{}",
                t!("errors.startup_failed", error = format!("{error:#}")).to_string()
            )
        });
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
        apply_component_theme(
            component_mode,
            &library.config.theme_style,
            Some(window),
            cx,
        );
        let skills = library.list().unwrap_or_default();
        let known_groups = library
            .groups()
            .into_iter()
            .map(|group| group.id)
            .collect::<HashSet<_>>();
        let selected = skills
            .first()
            .map(|skill| skill_storage_name(skill).to_string());
        crate::i18n::apply_language(library.config.language);
        let skill_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("common.search_skills").to_string())
        });
        let description_input = cx.new(|cx| {
            gpui_component::input::TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .submit_on_enter(true)
        });
        cx.subscribe_in(
            &description_input,
            window,
            |this, _, event: &InputEvent, _, cx| match event {
                InputEvent::PressEnter { shift: false, .. } | InputEvent::Blur => {
                    this.commit_description_edit(cx)
                }
                _ => {}
            },
        )
        .detach();
        let project_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("common.search_projects").to_string())
        });
        let tag_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("common.tag_name").to_string()));
        let group_name_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(t!("common.group_name").to_string()));
        let add_primary_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("add.source_placeholder").to_string())
        });
        let add_source_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    SourceChoice {
                        kind: AddKind::Npx,
                        label: "skills.sh / github".to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Claude,
                        label: t!("common.claude_plugin").to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Local,
                        label: t!("common.local_folder").to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Existing,
                        label: t!("common.existing_installations").to_string().into(),
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
        let initial_palette = Palette::current(component_mode.is_dark());
        let spinner_accent = cx.new(|_| SpinnerView {
            icon: "icons/rotate-cw.svg",
            color: initial_palette.accent,
            size: 14.,
        });
        let spinner_on_accent = cx.new(|_| SpinnerView {
            icon: "icons/rotate-cw.svg",
            color: initial_palette.on_primary,
            size: 12.,
        });
        let spinner_update = cx.new(|_| SpinnerView {
            icon: "icons/loader-circle.svg",
            color: initial_palette.secondary,
            size: icon_sm(),
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
                if this.add_flow.task.is_some() {
                    return;
                }
                this.add_flow.kind = *kind;
                let placeholder = match this.add_flow.kind {
                    AddKind::Claude => t!("add.plugin_placeholder").to_string(),
                    AddKind::Npx => t!("add.source_placeholder").to_string(),
                    AddKind::Local | AddKind::Existing => String::new(),
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
        let preset_store = crate::preset::PresetStore::open_in(&data_dir).unwrap_or_else(|error| {
            panic!(
                "{}",
                t!("errors.startup_failed", error = format!("{error:#}")).to_string()
            )
        });
        let preset_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("presets.name_placeholder").to_string())
        });
        let preset_skill_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("common.search_skills").to_string())
        });
        let preset_agents_md = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .folding(false)
                .soft_wrap(true)
        });
        cx.subscribe_in(
            &preset_name_input,
            window,
            |this, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.presets_view.error = None;
                    this.notify_dialog(cx);
                }
                InputEvent::PressEnter { .. } => this.commit_preset_name(cx),
                InputEvent::Focus | InputEvent::Blur => {}
            },
        )
        .detach();
        cx.subscribe_in(
            &preset_skill_search,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.notify_dialog(cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &preset_agents_md,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.save_preset_agents_md(cx);
                }
            },
        )
        .detach();
        let (tags, project_tags) = load_tag_states_from(&data_dir);
        let collapsed_groups = if library.config.collapsed_skill_groups_initialized {
            library
                .config
                .collapsed_skill_groups
                .iter()
                .cloned()
                .collect()
        } else {
            known_groups.clone()
        };
        let agents_settings = agents_settings::AgentsSettings::new(window, cx);
        Self {
            agents_settings,
            model: AppModel {
                library,
                skills,
                update_check: None,
                updating_skill: None,
                updating_batch: None,
                pending_updates: None,
            },
            shell: ShellState {
                page: Page::Skills,
                focus_handle: cx.focus_handle(),
                header_drag_armed: false,
                shell_layout,
                content_layout,
                spinner_accent,
                spinner_update,
                spinner_on_accent,
                notice: None,
                notice_generation: 0,
                system_dark,
                dialog_body: None,
                painting_dialog: false,
                page_body,
                data_location: None,
            },
            skills_view: SkillsState {
                expanded_description: None,
                editing_description: None,
                description_input,
                tab: DetailTab::Installs,
                selection: SkillSelection::new(selected),
                selected_file: PathBuf::from("SKILL.md"),
                skill_search,
                content_snapshot: RefCell::new(None),
                content_scroll: ScrollHandle::new(),
                content_tree_scroll: ScrollHandle::new(),
                list: ListState::new(0, ListAlignment::Top, px(100.)),
                list_rows: RefCell::new(Arc::from([])),
                selectable_text_handles: RefCell::new(BTreeMap::new()),
                known_groups,
                collapsed_groups,
                collapsed_content_directories: HashSet::new(),
            },
            projects_view: ProjectsState {
                open_project: None,
                global_project_view: true,
                batch_project: None,
                batch_selected: HashSet::new(),
                batch_filter: None,
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
                list_scroll: ScrollHandle::new(),
                skills_scroll: UniformListScrollHandle::new(),
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
                local_mode: crate::LocalInstallMode::Link,
                group_name: None,
                primary_input: add_primary_input,
                source_select: add_source_select,
            },
            install_flow: InstallFlowState {
                pending: None,
                error: None,
                modal: false,
                global: true,
                selected_targets: HashSet::new(),
                preset: None,
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
                drop_target: None,
                name_input: group_name_input,
                edit: None,
                delete_pending: None,
                delete_skills: false,
                move_skills: Vec::new(),
            },
            presets_view: PresetsState {
                selected: preset_store
                    .presets()
                    .first()
                    .map(|preset| preset.id.clone()),
                store: preset_store,
                tab: PresetTab::Skills,
                name_input: preset_name_input,
                edit: None,
                error: None,
                agents_md: preset_agents_md,
                agents_md_for: RefCell::new(None),
                selected_file: PathBuf::from("AGENTS.md"),
                collapsed_dirs: HashSet::new(),
                tree_scroll: ScrollHandle::new(),
                pending_conflict: None,
                skill_search: preset_skill_search,
                picked_skills: HashSet::new(),
                confirmation: None,
                statuses: RefCell::new(BTreeMap::new()),
                list_scroll: ScrollHandle::new(),
            },
        }
    }

    fn palette(&self) -> Palette {
        let dark = match self.model.library.config.theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => self.shell.system_dark,
        };
        let palette = Palette::current(dark);
        if self.shell.painting_dialog {
            palette.on_elevated()
        } else {
            palette
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

    /// Search match for skill lists. `query` is already trimmed and
    /// lowercased. Names and sources match anywhere; descriptions only at the
    /// start of a word, so "matt" finds `ask-matt` but not "formatting".
    fn skill_matches_query(&self, skill: &SkillSummary, query: &str) -> bool {
        if query.is_empty()
            || skill.record.name.to_lowercase().contains(query)
            || self
                .source_label(&skill.record.origin)
                .to_lowercase()
                .contains(query)
        {
            return true;
        }
        let description = skill.record.description.to_lowercase();
        if !query.is_ascii() {
            return description.contains(query);
        }
        description
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word.starts_with(query))
    }

    fn tags_for(&self, scope: TagScope) -> &TagState {
        match scope {
            TagScope::Skills => &self.tags_flow.skills,
        }
    }

    fn tags_for_mut(&mut self, scope: TagScope) -> &mut TagState {
        match scope {
            TagScope::Skills => &mut self.tags_flow.skills,
        }
    }

    fn persist_tags(&self) {
        let _ = save_tag_states_to(
            self.model.library.data_dir(),
            &self.tags_flow.skills,
            &self.tags_flow.projects,
        );
    }

    fn persist_collapsed_groups(&mut self) {
        self.model.library.config.collapsed_skill_groups =
            self.skills_view.collapsed_groups.iter().cloned().collect();
        self.model.library.config.collapsed_skill_groups_initialized = true;
        let _ = self
            .model
            .library
            .config
            .save_to(self.model.library.data_dir());
    }

    fn tag_filter_for(&self, scope: TagScope) -> Option<TagId> {
        match scope {
            TagScope::Skills => self.tags_flow.selected_skill_filter,
        }
    }

    fn set_tag_filter(&mut self, scope: TagScope, filter: Option<TagId>) {
        match scope {
            TagScope::Skills => self.tags_flow.selected_skill_filter = filter,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.model.skills = self.model.library.list().unwrap_or_default();
        let previous_collapsed_groups = self.skills_view.collapsed_groups.clone();
        let current_groups = self
            .model
            .library
            .groups()
            .into_iter()
            .map(|group| group.id)
            .collect::<HashSet<_>>();
        self.skills_view.collapsed_groups.extend(
            current_groups
                .difference(&self.skills_view.known_groups)
                .cloned(),
        );
        self.skills_view
            .collapsed_groups
            .retain(|id| current_groups.contains(id));
        self.skills_view.known_groups = current_groups;
        if self.skills_view.collapsed_groups != previous_collapsed_groups {
            self.persist_collapsed_groups();
        }
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
        self.reload_presets();
        cx.notify();
    }

    fn sync_spinner_palette(&self, cx: &mut Context<Self>) {
        let p = self.palette();
        self.shell.spinner_accent.update(cx, |spinner, cx| {
            spinner.color = p.accent;
            cx.notify();
        });
        self.shell.spinner_update.update(cx, |spinner, cx| {
            spinner.color = p.secondary;
            cx.notify();
        });
        self.shell.spinner_on_accent.update(cx, |spinner, cx| {
            spinner.color = p.on_primary;
            cx.notify();
        });
    }

    fn project_snapshot(
        &self,
        path: &PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<Arc<ProjectSnapshot>> {
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
        let library_skills = self.model.skills.clone();
        cx.spawn(async move |this, cx| {
            let snapshots = cx
                .background_executor()
                .spawn(async move {
                    pending
                        .into_iter()
                        .map(|path| {
                            let skills = project::list(&path, &library_dir).unwrap_or_default();
                            let mut snapshot = ProjectSnapshot::new(skills, &library_skills);
                            if let Some(root) =
                                crate::agents::installation_root(&path, &InstallTarget::universal())
                            {
                                snapshot.universal_agents =
                                    effective_skills::agents_reading_root(&path, &root);
                            }
                            (path, Arc::new(snapshot))
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
    ) -> Option<Arc<ProjectRenderSnapshot>> {
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
            return cached.map(|cache| cache.snapshot);
        }

        let generation = self.projects_view.scan_generation;
        if !self
            .projects_view
            .context_estimate_tasks
            .borrow()
            .get(path)
            .is_some_and(|running| *running == generation)
        {
            let project = self.project_snapshot(path, cx)?;
            self.projects_view
                .context_estimate_tasks
                .borrow_mut()
                .insert(path.clone(), generation);
            let path = path.clone();
            cx.spawn(async move |this, cx| {
                let worker_path = path.clone();
                let snapshot = cx
                    .background_executor()
                    .spawn(async move {
                        Arc::new(ProjectRenderSnapshot::new(
                            effective_skills::estimate_project(&worker_path),
                            &project,
                        ))
                    })
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
                            snapshot,
                        },
                    );
                    cx.notify();
                });
            })
            .detach();
        }

        cached.map(|cache| cache.snapshot)
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
        self.projects_view
            .project_snapshots
            .borrow_mut()
            .remove(&path);
        self.projects_view
            .project_snapshot_tasks
            .borrow_mut()
            .clear();
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
            .unwrap_or_else(|error| self.error_message(error));
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
        crate::i18n::apply_language(language);
        self.skills_view.skill_search.update(cx, |input, cx| {
            input.set_placeholder(t!("common.search_skills").to_string(), window, cx)
        });
        self.projects_view.project_search.update(cx, |input, cx| {
            input.set_placeholder(t!("common.search_projects").to_string(), window, cx)
        });
        self.tags_flow.name_input.update(cx, |input, cx| {
            input.set_placeholder(t!("common.tag_name").to_string(), window, cx)
        });
        self.groups_flow.name_input.update(cx, |input, cx| {
            input.set_placeholder(t!("common.group_name").to_string(), window, cx)
        });
        self.presets_view.name_input.update(cx, |input, cx| {
            input.set_placeholder(t!("presets.name_placeholder").to_string(), window, cx)
        });
        self.presets_view.skill_search.update(cx, |input, cx| {
            input.set_placeholder(t!("common.search_skills").to_string(), window, cx)
        });
        let placeholder = match self.add_flow.kind {
            AddKind::Npx => t!("add.source_placeholder").to_string(),
            AddKind::Claude => t!("add.plugin_placeholder").to_string(),
            AddKind::Local | AddKind::Existing => String::new(),
        };
        self.add_flow.primary_input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        self.add_flow.source_select.update(cx, |select, cx| {
            select.set_items(
                vec![
                    SourceChoice {
                        kind: AddKind::Npx,
                        label: "skills.sh / github".to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Claude,
                        label: t!("common.claude_plugin").to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Local,
                        label: t!("common.local_folder").to_string().into(),
                    },
                    SourceChoice {
                        kind: AddKind::Existing,
                        label: t!("common.existing_installations").to_string().into(),
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
        self.app.update(cx, |app, app_cx| {
            app.shell.painting_dialog = true;
            let body = match self.kind {
                DialogKind::AgentAdd => app.agent_add_modal(app_cx),
                DialogKind::AgentSource => app.agent_source_modal(app_cx),
                DialogKind::Add => app.add_skill_modal(window, app_cx),
                DialogKind::Install => app.install_modal(window, app_cx),
                DialogKind::Delete => app.delete_confirmation_modal(window, app_cx),
                DialogKind::Tags => app.tag_management_modal(window, app_cx),
                DialogKind::AssignTags => app.tag_assignment_modal(window, app_cx),
                DialogKind::Groups => app.group_management_modal(window, app_cx),
                DialogKind::DeleteGroup => app.group_delete_modal(app_cx),
                DialogKind::MoveGroup => app.move_group_modal(window, app_cx),
                DialogKind::PresetName => app.preset_name_modal(app_cx),
                DialogKind::PresetSkills => app.preset_skills_modal(app_cx),
                DialogKind::PresetConfirm => app.preset_confirm_modal(app_cx),
                DialogKind::DataLocation => app.data_location_modal(app_cx),
            };
            app.shell.painting_dialog = false;
            body
        })
    }
}

impl Render for PageBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app
            .update(cx, |app, app_cx| match app.shell.page {
                Page::Skills => app.skills_page(window, app_cx),
                Page::Projects => app.projects_page(window, app_cx),
                Page::Presets => app.presets_page(window, app_cx),
                Page::Settings => app.settings_page(window, app_cx),
            })
            .unwrap_or_else(|_| div())
    }
}

impl Render for KitterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let content = self.shell.page_body.clone();
        let mut root = kit::window_ground(p)
            .track_focus(&self.shell.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("escape")
                    && this.skills_view.selection.is_multiple()
                {
                    this.finish_skill_selection_mode(cx);
                    cx.stop_propagation();
                }
            }));
        let main = kit::main_area(p);
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
            let progress = self.model.updating_batch.clone();
            let mut card = kit::toast(p)
                .id("app-notice")
                .debug_selector(|| "app-notice".into())
                .absolute()
                .right(px(16.))
                .bottom(px(16.))
                .max_w(px(420.))
                .when(progress.is_some(), |card| card.min_w(px(240.)))
                .child(notice);
            if let Some(progress) = progress {
                if let Some(current) = progress.current.filter(|name| !name.is_empty()) {
                    let current = self
                        .model
                        .skills
                        .iter()
                        .find(|skill| skill_storage_name(skill) == current)
                        .map(|skill| skill.record.name.clone())
                        .unwrap_or(current);
                    card = card.child(
                        div()
                            .text_size(px(fs_small()))
                            .text_color(p.secondary)
                            .truncate()
                            .child(current),
                    );
                }
                if progress.total > 0 {
                    let ratio = progress.completed as f32 / progress.total as f32;
                    card = card.child(kit::progress_bar(p, ratio));
                }
            }
            root = root.child(card);
        }
        if let Some(body) = self.shell.dialog_body.clone() {
            let width = match body.read(cx).kind {
                DialogKind::AgentAdd => px(520.),
                DialogKind::AgentSource => px(420.),
                DialogKind::Add => px(520.),
                DialogKind::Install if self.install_flow.pending.is_some() => px(420.),
                DialogKind::Install => px(600.),
                DialogKind::Delete => px(420.),
                DialogKind::Tags | DialogKind::AssignTags => px(420.),
                DialogKind::Groups => px(420.),
                DialogKind::DeleteGroup => px(420.),
                DialogKind::MoveGroup => px(420.),
                DialogKind::PresetName | DialogKind::PresetConfirm => px(420.),
                DialogKind::PresetSkills => px(520.),
                DialogKind::DataLocation => px(420.),
            };
            root = root.child(
                kit::scrim(p)
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        kit::dialog_frame(p)
                            .id("dialog-frame")
                            .debug_selector(|| "dialog-frame".into())
                            .w(width)
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

    static LOCALE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn init(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
    }

    #[test]
    fn injected_data_directory_is_the_only_library_source() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
    fn collapsed_skill_groups_are_restored_from_config() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        let group_id = app.update(cx, |app, _| {
            let id = app
                .model
                .library
                .create_group("owner/repository")
                .unwrap()
                .id;
            app.skills_view.collapsed_groups.insert(id.clone());
            app.persist_collapsed_groups();
            id
        });

        let mut cx = TestAppContext::single();
        init(&mut cx);
        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        cx.read_entity(&app, |app, _| {
            assert!(app.skills_view.collapsed_groups.contains(&group_id));
            assert!(
                app.model
                    .library
                    .config
                    .collapsed_skill_groups
                    .contains(&group_id)
            );
        });

        app.update(cx, |app, _| {
            app.skills_view.collapsed_groups.remove(&group_id);
            app.persist_collapsed_groups();
        });
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        cx.read_entity(&app, |app, _| {
            assert!(!app.skills_view.collapsed_groups.contains(&group_id));
            assert!(app.model.library.config.collapsed_skill_groups_initialized);
        });
    }

    #[test]
    fn sidebar_navigation_switches_the_rendered_page() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
        // The list now includes default global Skills. Give this end-to-end
        // fixture its own home without changing the parallel test process.
        if std::env::var_os("KITTER_LARGE_PROJECT_UI_CHILD").is_none() {
            let isolated = tempfile::tempdir().unwrap();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "ui::e2e_tests::large_project_renders_and_scrolls_with_virtual_rows",
                ])
                .env("KITTER_LARGE_PROJECT_UI_CHILD", "1")
                .env("HOME", isolated.path());
            for key in [
                "CODEX_HOME",
                "CLAUDE_CONFIG_DIR",
                "GROK_HOME",
                "PI_CODING_AGENT_DIR",
                "XDG_CONFIG_HOME",
                "XDG_DATA_HOME",
                "OPENCODE_CONFIG_DIR",
                "OPENCODE_CONFIG",
                "OPENCODE_PROJECT_CONFIG",
                "COPILOT_SKILLS_DIRS",
                "AMP_SKILL_PATHS",
                "HERMES_HOME",
                "OPENCLAW_STATE_DIR",
                "CLAWDBOT_STATE_DIR",
                "OPENCLAW_CONFIG_PATH",
                "DIMCODE_HOME",
                "OMP_PROFILE",
                "PI_PROFILE",
            ] {
                command.env_remove(key);
            }
            assert!(command.status().unwrap().success());
            return;
        }
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let project = temp.path().join("fixture-project");
        fs::create_dir_all(&project).unwrap();
        for index in 1..=180 {
            let name = format!("fixture-skill-{index:03}");
            let skill = temp.path().join("sources").join(&name);
            fs::create_dir_all(&skill).unwrap();
            fs::write(
                skill.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Performance fixture {index}\n---\n"),
            )
            .unwrap();
            crate::project::install_from_path(
                &project,
                &skill,
                &name,
                &[
                    crate::InstallTarget::universal(),
                    crate::InstallTarget::from("codex"),
                    crate::InstallTarget::from("claude-code"),
                ],
            )
            .unwrap();
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
        let first_row = cx
            .debug_bounds("project-skill-fixture-skill-001")
            .expect("first virtual project row should be rendered");
        assert!(cx.debug_bounds("project-skill-fixture-skill-180").is_none());
        let scroll = cx
            .debug_bounds("project-skills-virtual-list")
            .expect("virtual project list should be rendered");
        assert_eq!(first_row.size.width, scroll.size.width);
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
    fn three_hundred_library_skills_virtualize_scroll_search_and_groups() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        let group = app.update(cx, |app, cx| {
            for index in 1..=300 {
                let name = format!("fixture-skill-{index:03}");
                let path = app.model.library.config.library_dir.join(&name);
                fs::create_dir_all(&path).unwrap();
                fs::write(
                    path.join("SKILL.md"),
                    format!("---\nname: {name}\ndescription: Fixture\n---\n"),
                )
                .unwrap();
            }
            let group = app.model.library.create_group("Fixture 2026").unwrap().id;
            app.refresh(cx);
            for skill in &mut app.model.skills {
                if matches!(
                    skill.record.name.as_str(),
                    "fixture-skill-001" | "fixture-skill-002"
                ) {
                    skill.record.group_id = Some(group.clone());
                }
            }
            app.skills_view.collapsed_groups.clear();
            group
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("skill-fixture-skill-001").is_some());
        assert!(cx.debug_bounds("skill-fixture-skill-300").is_none());
        assert!(cx.debug_bounds("skills-total-count:301").is_some());
        let group_count: &'static str = format!("skill-group-count-{group}:2").leak();
        assert!(cx.debug_bounds(group_count).is_some());
        let first = cx.debug_bounds("skill-fixture-skill-001").unwrap();
        cx.simulate_click(first.center(), Modifiers::none());
        cx.read_entity(&app, |app, _| {
            assert_eq!(
                app.skills_view.selection.primary(),
                Some("fixture-skill-001")
            )
        });
        let group_selector: &'static str = format!("sort-group-{group}").leak();
        let header = cx.debug_bounds(group_selector).unwrap();
        cx.simulate_click(header.center(), Modifiers::none());
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("skill-fixture-skill-001").is_none());
        assert!(cx.debug_bounds("skills-total-count:301").is_some());
        assert!(cx.debug_bounds(group_count).is_some());
        app.update(cx, |app, cx| {
            app.skills_view.list.scroll_to_end();
            cx.notify();
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("skill-fixture-skill-300").is_some());
        assert!(cx.debug_bounds("skill-fixture-skill-003").is_none());
        app.update_in(cx, |app, window, cx| {
            app.skills_view.skill_search.update(cx, |input, cx| {
                input.set_value("fixture-skill-002", window, cx)
            });
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("skill-fixture-skill-002").is_some());
        assert!(cx.debug_bounds("skill-fixture-skill-300").is_none());
        assert!(cx.debug_bounds("skills-total-count:1 / 301").is_some());
        let matched_count: &'static str = format!("skill-group-count-{group}:1 / 2").leak();
        assert!(cx.debug_bounds(matched_count).is_some());
        cx.read_entity(&app, |app, _| {
            assert!(app.skills_view.collapsed_groups.contains(&group))
        });
        app.update_in(cx, |app, window, cx| {
            app.skills_view.skill_search.update(cx, |input, cx| {
                input.set_value("no-matching-skill", window, cx)
            });
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("skills-total-count:0 / 301").is_some());
        assert!(cx.debug_bounds(matched_count).is_none());
    }

    #[test]
    fn refresh_drops_pending_project_scans_from_the_previous_generation() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let path = project.join(".agents/skills/fixture");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            "---\nname: fixture\ndescription: Fixture\n---\n",
        )
        .unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        app.update(cx, |app, cx| {
            assert!(app.context_estimate_snapshot(&project, cx).is_none());
            assert!(
                !app.projects_view
                    .context_estimate_tasks
                    .borrow()
                    .contains_key(&project)
            );
            app.refresh(cx);
        });
        cx.run_until_parked();
        cx.read_entity(&app, |app, _| {
            assert!(
                !app.projects_view
                    .project_snapshots
                    .borrow()
                    .contains_key(&project)
            );
            assert!(
                !app.projects_view
                    .context_estimates
                    .borrow()
                    .contains_key(&project)
            );
        });
        app.update(cx, |app, cx| {
            app.project_snapshot(&project, cx);
        });
        cx.run_until_parked();
        app.update(cx, |app, cx| {
            app.context_estimate_snapshot(&project, cx);
        });
        cx.run_until_parked();
        app.update(cx, |app, cx| {
            let first = app.context_estimate_snapshot(&project, cx).unwrap();
            let next = app.context_estimate_snapshot(&project, cx).unwrap();
            assert!(std::sync::Arc::ptr_eq(&first, &next));
            let rows = first.view(None).rows;
            assert!(std::sync::Arc::ptr_eq(&rows, &next.view(None).rows));
            app.refresh_context_estimate(project.clone(), cx);
            assert!(
                !app.projects_view
                    .project_snapshots
                    .borrow()
                    .contains_key(&project)
            );
            assert!(
                !app.projects_view
                    .context_estimates
                    .borrow()
                    .contains_key(&project)
            );
        });
    }

    #[test]
    fn groups_drag_in_both_views_and_keep_the_saved_order() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::MouseButton;
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        let (a, b) = app.update(cx, |app, cx| {
            let a = app.model.library.create_group("A").unwrap().id;
            let b = app.model.library.create_group("B").unwrap().id;
            app.open_group_dialog(cx);
            (a, b)
        });
        let a_row: &'static str = format!("sort-group-{a}").leak();
        let b_row: &'static str = format!("sort-group-{b}").leak();
        let a_handle: &'static str = format!("drag-group-{a}").leak();
        for management in [true, false] {
            if !management {
                app.update(cx, |app, cx| app.close_dialog(cx));
            }
            cx.refresh().unwrap();
            let row = cx.debug_bounds(a_row).unwrap();
            cx.simulate_mouse_move(row.center(), None, Modifiers::none());
            cx.refresh().unwrap();
            let start = if management {
                cx.debug_bounds(a_handle).unwrap().center()
            } else {
                assert!(
                    cx.debug_bounds(a_handle).is_none(),
                    "list must not have a drag handle or its spacing"
                );
                row.center()
            };
            if management {
                let handle = cx.debug_bounds(a_handle).unwrap();
                assert_eq!(handle.size.width, px(24.));
                assert_eq!(handle.origin.x, row.origin.x);
                assert_eq!(
                    cx.debug_bounds(a_row).unwrap(),
                    row,
                    "hover must not shift the row"
                );
            }
            let target = cx.debug_bounds(b_row).unwrap();
            let end = point(
                target.center().x,
                target.center().y + if management { px(5.) } else { px(-5.) },
            );
            cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                start + point(px(5.), px(0.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.refresh().unwrap();
            cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
            cx.refresh().unwrap();
            assert_eq!(
                cx.debug_bounds("group-drop-line-Management").is_some(),
                management
            );
            assert_eq!(
                cx.debug_bounds("group-drop-line-List").is_some(),
                !management
            );
            cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
            cx.refresh().unwrap();
            app.update(cx, |app, _| {
                let ids = app
                    .model
                    .library
                    .groups()
                    .into_iter()
                    .map(|group| group.id)
                    .collect::<Vec<_>>();
                assert_eq!(
                    ids,
                    if management {
                        vec![b.clone(), a.clone()]
                    } else {
                        vec![a.clone(), b.clone()]
                    }
                );
                assert!(app.skills_view.collapsed_groups.is_empty());
            });
        }
    }

    #[test]
    fn creating_group_inside_add_keeps_selection_and_dialog() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use super::{DialogKind, GroupEdit};
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        app.update_in(cx, |app, window, cx| {
            app.open_add_dialog(window, cx);
            app.add_flow.selected.extend(["one".into(), "two".into()]);
            app.notify_dialog(cx);
        });
        cx.refresh().unwrap();
        let selector = cx.debug_bounds("add-group-control").unwrap();
        cx.simulate_click(
            point(selector.center().x, selector.bottom() - px(12.)),
            Modifiers::none(),
        );
        cx.refresh().unwrap();
        let create = cx.debug_bounds("add-group-create").unwrap();
        cx.simulate_click(create.center(), Modifiers::none());
        cx.refresh().unwrap();
        cx.simulate_input("Team");
        cx.simulate_keystrokes("enter");
        app.update_in(cx, |app, window, cx| {
            assert!(matches!(
                app.shell.dialog_body.as_ref().unwrap().read(cx).kind,
                DialogKind::Add
            ));
            assert_eq!(app.add_flow.group_name.as_deref(), Some("Team"));
            assert_eq!(app.add_flow.selected.len(), 2);
            assert_eq!(app.model.library.groups().len(), 1);
            app.start_group_edit(GroupEdit::Create, window, cx);
            app.groups_flow
                .name_input
                .update(cx, |input, cx| input.set_value("Team", window, cx));
            app.commit_group_edit(cx);
            assert!(app.groups_flow.edit.is_some());
            assert!(app.tags_flow.error.is_some());
            assert_eq!(app.model.library.groups().len(), 1);
        });
    }

    #[test]
    fn busy_add_dialog_blocks_changes_and_recovers_when_idle() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use super::{AddTask, Language};
        use gpui::Focusable;
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let (app, cx) = cx.add_window_view({
            let data_dir = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        app.update_in(cx, |app, window, cx| {
            app.set_language(Language::En, window, cx);
            app.open_add_dialog(window, cx);
            app.add_flow.primary_input.update(cx, |input, cx| {
                input.set_value("https://github.com/mattpocock/skills", window, cx);
                input.focus_handle(cx).focus(window, cx);
            });
            app.add_flow.selected.extend(["one".into(), "two".into()]);
            app.add_flow.task = Some(AddTask::Scanning);
            app.notify_dialog(cx);
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("scan-progress").is_some());
        cx.simulate_keystrokes("x");
        for selector in ["close-add-modal", "cancel-add"] {
            let bounds = cx.debug_bounds(selector).unwrap();
            cx.simulate_click(bounds.center(), Modifiers::none());
        }
        app.update_in(cx, |app, window, cx| {
            assert_eq!(
                app.add_flow.primary_input.read(cx).value().as_ref(),
                "https://github.com/mattpocock/skills"
            );
            assert!(
                !app.add_flow
                    .primary_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            app.set_add_kind(AddKind::Local, window, cx);
            assert!(app.add_flow.kind == AddKind::Npx);
            app.close_dialog(cx);
            assert!(app.shell.dialog_body.is_some());
            app.add_flow.task = Some(AddTask::Importing);
            app.notify_dialog(cx);
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("scan-progress").is_none());
        app.update(cx, |app, cx| {
            app.add_flow.task = None;
            app.notify_dialog(cx);
        });
        cx.refresh().unwrap();
        let bounds = cx.debug_bounds("close-add-modal").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.read_entity(&app, |app, _| assert!(app.shell.dialog_body.is_none()));
    }

    #[test]
    fn local_folder_picker_scans_inside_the_isolated_window() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
            .expect("scanned skill row should be rendered");
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

    #[test]
    fn startup_update_check_is_silent_and_restarts_use_the_saved_limit() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        let (app, cx) = cx.add_window_view({
            let data = data.clone();
            move |window, cx| KitterApp::new_in(data, window, cx)
        });
        app.update(cx, |app, cx| {
            app.start_background_update_check(cx);
            assert!(app.model.update_check.is_some());
            assert!(app.shell.notice.is_none());
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        let timestamp = fs::read(data.join("update-check.json")).unwrap();
        app.update(cx, |app, cx| {
            assert!(app.model.update_check.is_none());
            assert!(app.shell.notice.is_none());
            app.start_background_update_check(cx);
            app.request_skill_updates(super::SkillUpdateRequest::All, cx);
            assert!(app.model.pending_updates.is_some());
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        app.update(cx, |app, _| {
            assert!(app.model.update_check.is_none());
            assert!(app.model.pending_updates.is_none());
            assert_eq!(
                app.shell.notice.as_deref(),
                Some(t!("updates.none").as_ref())
            );
        });
        assert_eq!(fs::read(data.join("update-check.json")).unwrap(), timestamp);
    }

    #[test]
    fn update_all_fetches_new_content_without_cached_update_flags() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let old = "---\nname: demo\ndescription: Fixture\n---\nOld instructions\n";
        let new = "---\nname: demo\ndescription: Fixture\n---\nNew instructions\n";
        fs::write(repo.join("SKILL.md"), old).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env(
                    "GIT_CONFIG_GLOBAL",
                    if cfg!(windows) { "NUL" } else { "/dev/null" },
                )
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "initial",
        ]);
        let data = temp.path().join("data");
        let (app, cx) = cx.add_window_view({
            let data = data.clone();
            move |window, cx| KitterApp::new_in(data, window, cx)
        });
        let installed = app.update(cx, |app, cx| {
            app.model
                .library
                .import(
                    &repo,
                    crate::SkillRecord {
                        name: "demo".into(),
                        storage_name: String::new(),
                        description: String::new(),
                        custom_description: None,
                        origin: crate::SkillOrigin::Git {
                            repository: repo.to_string_lossy().into_owned(),
                            subdir: None,
                        },
                        update_available: false,
                        group_id: None,
                        last_operated_at: 0,
                    },
                )
                .unwrap();
            app.refresh(cx);
            app.model.library.skill_path("demo").unwrap()
        });
        fs::write(repo.join("SKILL.md"), new).unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "updated",
        ]);
        // A recent automatic-check attempt must not prevent an explicit update.
        fs::write(
            data.join("update-check.json"),
            serde_json::to_vec(
                &std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
            .unwrap(),
        )
        .unwrap();
        app.update(cx, |app, cx| {
            assert!(!app.model.library.record("demo").unwrap().update_available);
            app.start_background_update_check(cx);
            app.request_skill_updates(super::SkillUpdateRequest::All, cx);
            assert!(app.model.pending_updates.is_some());
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        cx.refresh().unwrap();
        cx.read_entity(&app, |app, _| {
            assert!(app.model.pending_updates.is_none());
            assert!(app.model.updating_batch.is_none());
            assert_eq!(fs::read_to_string(installed.join("SKILL.md")).unwrap(), new);
            assert!(!app.model.library.record("demo").unwrap().update_available);
        });
    }

    #[test]
    fn updates_are_visible_in_collapsed_groups_and_selected_skill_details() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Fixture\nmetadata:\n  version: \"1.2.0\"\n---\n",
        )
        .unwrap();
        let (app, cx) = cx.add_window_view({
            let data = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data, window, cx)
        });
        let group = app.update(cx, |app, cx| {
            let group = app.model.library.ensure_group("Example").unwrap();
            app.model
                .library
                .import(
                    &source,
                    crate::SkillRecord {
                        name: "demo".into(),
                        storage_name: String::new(),
                        description: String::new(),
                        custom_description: None,
                        origin: crate::SkillOrigin::Unknown,
                        update_available: true,
                        group_id: Some(group.clone()),
                        last_operated_at: 0,
                    },
                )
                .unwrap();
            app.refresh(cx);
            app.skills_view.selection.select_one("demo".into());
            app.skills_view.collapsed_groups.insert(group.clone());
            cx.notify();
            group
        });
        let group_selector: &'static str =
            Box::leak(format!("update-group-dot-{group}").into_boxed_str());
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds(group_selector).is_some());
        assert!(cx.debug_bounds("update-dot-demo").is_none());
        assert!(cx.debug_bounds("update-skill").is_some());
        assert!(cx.debug_bounds("update-skill-refresh").is_some());
        let update_width = cx.debug_bounds("update-skill").unwrap().size.width;
        assert!(cx.debug_bounds("skill-detail-version").is_some());
        app.update(cx, |app, cx| {
            app.model.updating_skill = Some("demo".into());
            cx.notify();
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("update-skill-progress").is_some());
        assert!(cx.debug_bounds("update-skill-refresh").is_none());
        assert_eq!(
            cx.debug_bounds("update-skill").unwrap().size.width,
            update_width
        );
        app.update(cx, |app, cx| {
            app.model.updating_skill = Some("another-skill".into());
            cx.notify();
        });
        cx.refresh().unwrap();
        assert!(cx.debug_bounds("update-skill-progress").is_none());
        app.update(cx, |app, cx| {
            app.model.updating_skill = None;
            assert_eq!(
                app.selected_skill().unwrap().version.as_deref(),
                Some("1.2.0")
            );
            app.skills_view.collapsed_groups.clear();
            cx.notify();
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("update-dot-demo").is_some());
        assert!(cx.debug_bounds(group_selector).is_none());
        app.update(cx, |app, cx| {
            app.model
                .library
                .set_update_available("demo", false)
                .unwrap();
            fs::write(
                app.model
                    .library
                    .skill_path("demo")
                    .unwrap()
                    .join("SKILL.md"),
                "---\nname: demo\ndescription: No declared version\n---\n",
            )
            .unwrap();
            app.refresh(cx);
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.debug_bounds("update-dot-demo").is_none());
        assert!(cx.debug_bounds("update-skill").is_none());
        assert!(cx.debug_bounds("skill-detail-version").is_none());
    }

    #[test]
    fn installation_conflicts_require_confirmation_and_back_keeps_files() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use super::Language;
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Test\n---\n",
        )
        .unwrap();
        let project = temp.path().join("project");
        let destination = project.join(".agents/skills/demo");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("keep"), "old files").unwrap();
        let (app, cx) = cx.add_window_view({
            let data = temp.path().join("data");
            move |window, cx| KitterApp::new_in(data, window, cx)
        });
        app.update_in(cx, |app, window, cx| {
            app.set_language(Language::ZhCn, window, cx);
            let scan = crate::source::scan_local(&source).unwrap();
            scan.import_selected(
                &mut app.model.library,
                &std::collections::HashSet::from(["demo".to_string()]),
                None,
            )
            .unwrap();
            app.refresh(cx);
            app.skills_view.selection.select_one("demo".to_string());
            app.projects_view.global_project_view = false;
            app.projects_view.open_project = Some(project.clone());
            app.open_install_dialog(window, cx);
            app.install_flow
                .selected_targets
                .insert(crate::InstallTarget::universal());
            app.notify_dialog(cx);
        });
        let click = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
            cx.refresh().unwrap();
            cx.run_until_parked();
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("missing {selector}"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            cx.refresh().unwrap();
            cx.run_until_parked();
        };
        click(cx, "confirm-install");
        assert!(cx.debug_bounds("confirm-install-replace").is_some());
        assert_eq!(
            cx.debug_bounds("dialog-frame").unwrap().size.width,
            gpui::px(420.)
        );
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "old files"
        );
        click(cx, "cancel-install-replace");
        assert!(cx.debug_bounds("confirm-install").is_some());
        assert_eq!(
            cx.debug_bounds("dialog-frame").unwrap().size.width,
            gpui::px(600.)
        );
        assert!(destination.join("keep").is_file());
        click(cx, "confirm-install");
        click(cx, "confirm-install-replace");
        assert_eq!(
            destination.canonicalize().unwrap(),
            source.canonicalize().unwrap()
        );
        assert!(source.join("SKILL.md").is_file());
        cx.read_entity(&app, |app, _| assert!(app.shell.dialog_body.is_none()));
    }

    #[test]
    fn preset_workflow_runs_through_the_ui() {
        let _locale_guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use super::{DialogKind, Language, PresetEdit};
        use gpui::{Focusable, VisualTestContext};
        let mut cx = TestAppContext::single();
        init(&mut cx);
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("kitter-data");
        let source = temp.path().join("skills");
        for name in ["alpha", "beta", "gamma"] {
            fs::create_dir_all(source.join(name)).unwrap();
            fs::write(
                source.join(name).join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Fixture\n---\n"),
            )
            .unwrap();
        }
        let project = temp.path().join("web");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("AGENTS.md"), "# Mine\n").unwrap();

        let (app, cx) = cx.add_window_view({
            let data_dir = data_dir.clone();
            move |window, cx| KitterApp::new_in(data_dir, window, cx)
        });
        let project = app.update_in(cx, |app, window, cx| {
            app.set_language(Language::ZhCn, window, cx);
            for name in ["alpha", "beta", "gamma"] {
                app.model
                    .library
                    .import(
                        &source.join(name),
                        crate::SkillRecord {
                            name: name.into(),
                            storage_name: String::new(),
                            description: "Fixture".into(),
                            custom_description: None,
                            origin: crate::SkillOrigin::Unknown,
                            update_available: false,
                            group_id: None,
                            last_operated_at: 0,
                        },
                    )
                    .unwrap();
            }
            app.model.library.config.touch_project(&project);
            app.model.library.save().unwrap();
            app.refresh(cx);
            app.model.library.config.project_paths()[0].clone()
        });
        let skills_dir = project.join(".agents/skills");
        let agents_md = || fs::read_to_string(project.join("AGENTS.md")).unwrap();
        let rendered = |cx: &mut VisualTestContext, selector: &'static str| {
            cx.refresh().unwrap();
            cx.run_until_parked();
            cx.debug_bounds(selector).is_some()
        };
        let click = |cx: &mut VisualTestContext, selector: &'static str| {
            cx.refresh().unwrap();
            cx.run_until_parked();
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} should be rendered"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            cx.refresh().unwrap();
            cx.run_until_parked();
        };

        // Create a preset and fill it with skills.
        click(cx, "nav-presets");
        assert!(rendered(cx, "empty-new-preset"));
        click(cx, "add-preset");
        cx.simulate_input("前端");
        click(cx, "confirm-preset-name");
        let id = cx.read_entity(&app, |app, _| {
            let preset = app
                .selected_preset()
                .expect("new preset should be selected");
            assert_eq!(preset.name, "前端");
            preset.id.clone()
        });
        click(cx, "add-preset-skill");
        click(cx, "pick-preset-skill-alpha");
        click(cx, "pick-preset-skill-beta");
        click(cx, "confirm-preset-skills");
        cx.read_entity(&app, |app, _| {
            assert_eq!(app.selected_preset().unwrap().skills, ["alpha", "beta"]);
            assert!(app.shell.dialog_body.is_none());
        });

        // Typing into the AGENTS.md tab saves the template.
        click(cx, "preset-tab-agents-md");
        app.update_in(cx, |app, window, cx| {
            app.presets_view
                .agents_md
                .focus_handle(cx)
                .focus(window, cx);
        });
        cx.simulate_input("Use pnpm.");
        cx.run_until_parked();
        cx.read_entity(&app, |app, _| {
            assert_eq!(app.presets_view.store.agents_md(&id).unwrap(), "Use pnpm.");
        });

        // Install it from the project page.
        click(cx, "nav-projects");
        click(cx, String::leak(format!("project-{}", project.display())));
        click(cx, "project-skills-tab-presets");
        click(cx, "install-project-preset");
        click(cx, "install-project-preset-option-0");
        cx.read_entity(&app, |app, _| {
            assert_eq!(app.install_flow.preset.as_deref(), Some(id.as_str()));
            assert!(!app.install_flow.global);
            assert_eq!(app.projects_view.open_project.as_ref(), Some(&project));
        });
        if !cx.read_entity(&app, |app, _| {
            app.install_flow
                .selected_targets
                .contains(&crate::InstallTarget::universal())
        }) {
            click(cx, "target-universal");
        }
        click(cx, "confirm-install");
        assert!(skills_dir.join("alpha").exists() && skills_dir.join("beta").exists());
        assert_eq!(
            agents_md(),
            format!(
                "# Mine\n\n<!-- kitter:preset:{id} -->\nUse pnpm.\n<!-- /kitter:preset:{id} -->\n"
            )
        );
        assert!(rendered(cx, "open-project-preset-0"));
        assert!(!rendered(cx, "sync-project-preset-0"));

        // Editing the preset asks the project to sync.
        click(cx, "open-project-preset-0");
        click(cx, "preset-tab-skills");
        click(cx, "add-preset-skill");
        click(cx, "pick-preset-skill-gamma");
        click(cx, "confirm-preset-skills");
        click(cx, "preset-tab-projects");
        assert!(rendered(cx, "sync-all-preset-projects"));
        click(cx, "sync-preset-project-0");
        assert!(skills_dir.join("gamma").exists());
        assert!(!rendered(cx, "sync-preset-project-0"));

        // A block edited in the project is only overwritten after confirmation.
        fs::write(
            project.join("AGENTS.md"),
            agents_md().replace("Use pnpm.", "Use npm."),
        )
        .unwrap();
        app.update(cx, |app, cx| app.refresh(cx));
        click(cx, "sync-preset-project-0");
        cx.read_entity(&app, |app, cx| {
            assert!(matches!(
                app.shell.dialog_body.as_ref().unwrap().read(cx).kind,
                DialogKind::PresetConfirm
            ));
        });
        assert!(agents_md().contains("Use npm."));
        click(cx, "confirm-overwrite-preset");
        assert!(agents_md().contains("Use pnpm."));

        // Rename, then remove it from the project.
        app.update_in(cx, |app, window, cx| {
            app.open_preset_name_dialog(PresetEdit::Rename(id.clone()), window, cx);
            app.presets_view
                .name_input
                .update(cx, |input, cx| input.set_value("", window, cx));
        });
        cx.simulate_input("Web");
        click(cx, "confirm-preset-name");
        cx.read_entity(&app, |app, _| {
            assert_eq!(app.selected_preset().unwrap().name, "Web");
        });
        click(cx, "nav-projects");
        click(cx, "remove-project-preset-0");
        click(cx, "confirm-uninstall-preset");
        assert_eq!(agents_md(), "# Mine\n");
        for name in ["alpha", "beta", "gamma"] {
            assert!(!skills_dir.join(name).exists());
        }

        // Delete the preset.
        click(cx, "nav-presets");
        click(cx, "delete-preset");
        click(cx, "confirm-delete-preset");
        cx.read_entity(&app, |app, _| {
            assert!(app.presets_view.store.presets().is_empty());
        });
        assert!(rendered(cx, "empty-new-preset"));
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

fn open_main_window(cx: &mut App) {
    cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Kitter".into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: cfg!(target_os = "macos").then(|| point(px(16.), px(17.))),
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
            let view = main_view(window, cx);
            cx.new(|cx| main_root(view, window, cx))
        },
    )
    .expect("failed to open Kitter window");
}

fn main_view(window: &mut Window, cx: &mut App) -> Entity<KitterApp> {
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
    view
}

// Kitter uses the platform window decorations. The component Root's
// client-side border adds a full-window mouse-move layer that is only needed
// by borderless Linux windows.
fn main_root(view: Entity<KitterApp>, window: &mut Window, cx: &mut Context<Root>) -> Root {
    Root::new(view, window, cx)
        .bordered(false)
        .bg(cx.theme().background)
}

/// Rebuild every window from the current data folder.
fn reload_windows(cx: &mut App, after: impl Fn(&mut KitterApp, &mut Context<KitterApp>) + 'static) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, cx| {
            let mut app = None;
            window.replace_root(cx, |window, cx| {
                let view = main_view(window, cx);
                app = Some(view.clone());
                main_root(view, window, cx)
            });
            if let Some(app) = app {
                app.update(cx, |app, cx| after(app, cx));
            }
        });
    }
}

pub fn run() {
    let app = gpui_platform::application().with_assets(crate::assets::Assets);
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        }
    });
    app.run(|cx: &mut App| {
        cx.on_action(quit);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_app_identity("dev.kitter.app", "Kitter");
        gpui_component::init(cx);
        crate::assets::register_fonts(cx).expect("failed to register fonts");
        // Kitter has nothing to do in the background, so closing the window quits.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        open_main_window(cx);
    });
}
