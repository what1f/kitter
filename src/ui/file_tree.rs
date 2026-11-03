//! Collapsible file tree shared by the skill content view and preset
//! instructions. Rows come back without handlers so each caller decides
//! what clicking a file or folder does.

use super::*;

pub(super) struct TreeRow {
    pub path: PathBuf,
    pub directory: bool,
    pub row: Stateful<Div>,
}

/// Builds the visible rows for `files` (relative paths). `pinned` is listed
/// before everything else; children of `collapsed` folders are hidden.
pub(super) fn rows(
    p: Palette,
    id_prefix: &str,
    files: &[PathBuf],
    pinned: Option<&Path>,
    selected: &Path,
    collapsed: &HashSet<PathBuf>,
) -> Vec<TreeRow> {
    let mut directories = Vec::<PathBuf>::new();
    for file in files {
        let mut parent = file.parent();
        while let Some(path) = parent {
            if !path.as_os_str().is_empty() && !directories.iter().any(|item| item == path) {
                directories.push(path.to_path_buf());
            }
            parent = path.parent();
        }
    }
    let mut entries = directories
        .into_iter()
        .map(|path| (path, true))
        .chain(files.iter().cloned().map(|path| (path, false)))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        let pinned_first = |entry: &(PathBuf, bool)| Some(entry.0.as_path()) != pinned;
        pinned_first(left)
            .cmp(&pinned_first(right))
            .then_with(|| {
                left.0
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right.0.to_string_lossy().to_lowercase())
            })
            .then_with(|| right.1.cmp(&left.1))
    });
    let hidden = |path: &Path| {
        path.ancestors()
            .skip(1)
            .filter(|path| !path.as_os_str().is_empty())
            .any(|path| collapsed.contains(path))
    };
    entries
        .into_iter()
        .filter(|(path, _)| !hidden(path))
        .map(|(path, directory)| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let row = if directory {
                let depth = path.components().count().saturating_sub(1) as f32;
                let folded = collapsed.contains(&path);
                kit::list_row(
                    p,
                    ElementId::Name(format!("{id_prefix}directory-{}", path.display()).into()),
                    false,
                )
                .w_auto()
                .h(px(row_1()))
                .mx(px(8.))
                .pl(px(4. + depth * 14.))
                .gap(px(6.))
                .text_color(p.secondary)
                .child(slot(14.).child(KitterApp::icon(
                    if folded {
                        "icons/chevron-right.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    14.,
                    p.muted,
                )))
                .child(slot(18.).child(KitterApp::icon(
                    if folded {
                        "icons/folder.svg"
                    } else {
                        "icons/folder-open.svg"
                    },
                    icon_md(),
                    p.secondary,
                )))
                .child(label(name))
            } else {
                let depth =
                    path.parent()
                        .map_or(0, |parent| parent.components().count()) as f32;
                kit::list_row(
                    p,
                    ElementId::Name(format!("{id_prefix}file-{}", path.display()).into()),
                    path == selected,
                )
                .w_auto()
                .mx(px(8.))
                .pl(px(4. + depth * 14.))
                .gap(px(6.))
                .child(slot(14.))
                .child(slot(18.).child(KitterApp::icon("icons/file.svg", icon_md(), p.muted)))
                .child(label(name))
            };
            TreeRow {
                path,
                directory,
                row,
            }
        })
        .collect()
}

fn slot(size: f32) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

fn label(name: String) -> Div {
    div()
        .min_w_0()
        .flex_1()
        .font_family(mono())
        .text_size(px(fs_small()))
        .truncate()
        .child(name)
}
