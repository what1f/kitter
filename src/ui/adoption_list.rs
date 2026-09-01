//! Only visible rows are materialized by GPUI's variable-height list.
//! Source headers and individual references are rows too, so expanding a
//! heavily reused Skill never creates thousands of child elements at once.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdoptionRow {
    Header(usize),
    Candidate(usize),
    Disclosure(usize),
    Reference(usize, usize),
}

fn rows_for(scan: &adoption::AdoptionScan) -> Vec<AdoptionRow> {
    let mut rows = Vec::with_capacity(scan.candidates.len() * 3);
    let mut previous = String::new();
    for (index, candidate) in scan.candidates.iter().enumerate() {
        let source = candidate.origin.source().key();
        if previous != source {
            rows.push(AdoptionRow::Header(index));
            previous = source;
        }
        rows.push(AdoptionRow::Candidate(index));
        if !candidate.references.is_empty() {
            rows.push(AdoptionRow::Disclosure(index));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{AdoptionRow, rows_for};
    use crate::adoption;
    use gpui::{ListAlignment, ListState, px};
    use std::path::PathBuf;

    #[test]
    fn large_list_keeps_only_lightweight_rows_and_references_start_collapsed() {
        let candidates = (0..10_000)
            .map(|index| {
                let mut candidate = adoption::AdoptionCandidate::fixture(
                    &format!("/fixture/{index}"),
                    crate::SkillOrigin::Npx {
                        repository: "fixture/repo".into(),
                        skill: format!("skill-{index}"),
                        source_hash: None,
                    },
                );
                candidate.name = format!("skill-{index}");
                candidate.references = (0..if index == 0 { 10_000 } else { 1 })
                    .map(|reference| adoption::SkillReference {
                        path: PathBuf::from(format!(
                            "/project-{reference}/.agents/skills/skill-{index}"
                        )),
                        source: candidate.source.clone(),
                        kind: adoption::ReferenceKind::Link,
                        original_target: Some(candidate.source.clone()),
                    })
                    .collect();
                candidate
            })
            .collect();
        let scan = adoption::AdoptionScan::new(candidates, vec![]);
        let mut rows = rows_for(&scan);
        assert_eq!(rows.len(), 20_001); // one source header, candidate + disclosure per Skill
        assert!(!rows.iter().any(|r| matches!(r, AdoptionRow::Reference(..))));
        let state = ListState::new(rows.len(), ListAlignment::Top, px(80.));
        // Expansion adds row indexes, never child elements, and keeps later rows intact.
        rows.splice(
            3..3,
            (0..10_000).map(|reference| AdoptionRow::Reference(0, reference)),
        );
        state.splice(3..3, 10_000);
        assert_eq!(state.item_count(), rows.len());
        assert_eq!(rows[10_003], AdoptionRow::Candidate(1));
        rows.drain(3..10_003);
        state.splice(3..10_003, 0);
        assert_eq!(rows, rows_for(&scan));
        assert_eq!(state.item_count(), 20_001);
    }
}

impl KitterApp {
    pub(super) fn reset_adoption_rows(&mut self) {
        self.add_flow.adoption_expanded.clear();
        self.add_flow.adoption_rows = self
            .add_flow
            .adoption_scan
            .as_ref()
            .map(|scan| rows_for(scan))
            .unwrap_or_default();
        self.add_flow
            .adoption_list
            .reset(self.add_flow.adoption_rows.len());
    }

    pub(super) fn adoption_row(&self, position: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.add_flow.adoption_rows.get(position).copied() else {
            return div().into_any_element();
        };
        let index = match row {
            AdoptionRow::Header(i)
            | AdoptionRow::Candidate(i)
            | AdoptionRow::Disclosure(i)
            | AdoptionRow::Reference(i, _) => i,
        };
        let Some(candidate) = self
            .add_flow
            .adoption_scan
            .as_ref()
            .and_then(|s| s.candidates.get(index))
        else {
            return div().into_any_element();
        };
        let p = self.palette();
        let base = div().w_full().rounded(px(RADIUS_CONTROL)).overflow_hidden();
        match row {
            AdoptionRow::Header(_) => base
                .h(px(32.))
                .px(px(11.))
                .flex()
                .items_center()
                .text_size(px(12.))
                .text_color(p.secondary)
                .child(candidate.origin.label())
                .into_any_element(),
            AdoptionRow::Candidate(_) => {
                let id = candidate.id.clone();
                let selected = self.add_flow.selected.contains(&id);
                let disabled = self.add_flow.task.is_some() || candidate.issue.is_some();
                let source_path = display_path(&candidate.source);
                let tooltip = source_path.clone();
                let conflict = self
                    .add_flow
                    .adoption_scan
                    .as_ref()
                    .is_some_and(|scan| scan.has_conflict(&candidate.identity()));
                let label = if let Some(issue) = &candidate.issue {
                    issue.clone()
                } else if conflict {
                    self.tr("选择版本", "Choose version").into()
                } else if candidate.existing_storage.is_some() {
                    self.tr("已托管", "Managed").into()
                } else if self.uses_english() {
                    format!("{} references", candidate.references.len())
                } else {
                    format!("{} 处引用", candidate.references.len())
                };
                base.id(ElementId::Name(format!("adoption-row-{index}").into()))
                    .h(px(54.))
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .bg(if selected { p.selected } else { p.surface })
                    .child(
                        Checkbox::new(ElementId::Name(format!("adoption-check-{index}").into()))
                            .checked(selected),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(13.))
                                    .truncate()
                                    .child(candidate.name.clone()),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("adoption-source-{index}").into()))
                                    .mt(px(2.))
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(source_path)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(tooltip.clone()).build(window, cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(label),
                    )
                    .when(!disabled, |row| {
                        row.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(scan) = &this.add_flow.adoption_scan {
                                    scan.select(&mut this.add_flow.selected, &id);
                                }
                                this.notify_dialog(cx);
                            }))
                    })
                    .into_any_element()
            }
            AdoptionRow::Disclosure(_) => {
                let id = candidate.id.clone();
                let expanded = self.add_flow.adoption_expanded.contains(&id);
                base.id(ElementId::Name(format!("adoption-expand-{index}").into()))
                    .h(px(24.))
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .border_b_1()
                    .border_color(p.border)
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .cursor_pointer()
                    .child(Self::icon(
                        if expanded {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        },
                        11.,
                        p.muted,
                    ))
                    .child(self.tr("引用位置", "References"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let Some(candidate) = this
                            .add_flow
                            .adoption_scan
                            .as_ref()
                            .and_then(|s| s.candidates.get(index))
                        else {
                            return;
                        };
                        let count = candidate.references.len();
                        if this.add_flow.adoption_expanded.remove(&id) {
                            this.add_flow
                                .adoption_rows
                                .drain(position + 1..position + 1 + count);
                            this.add_flow
                                .adoption_list
                                .splice(position + 1..position + 1 + count, 0);
                        } else {
                            this.add_flow.adoption_expanded.insert(id.clone());
                            this.add_flow.adoption_rows.splice(
                                position + 1..position + 1,
                                (0..count)
                                    .map(|reference| AdoptionRow::Reference(index, reference)),
                            );
                            this.add_flow
                                .adoption_list
                                .splice(position + 1..position + 1, count);
                        }
                        this.notify_dialog(cx);
                    }))
                    .into_any_element()
            }
            AdoptionRow::Reference(_, reference) => {
                let path = display_path(&candidate.references[reference].path);
                let tooltip = path.clone();
                base.id(ElementId::Name(
                    format!("adoption-reference-{index}-{reference}").into(),
                ))
                .h(px(24.))
                .px(px(22.))
                .font_family(MONO)
                .text_size(px(11.))
                .text_color(p.muted)
                .truncate()
                .child(path)
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .into_any_element()
            }
        }
    }
}
