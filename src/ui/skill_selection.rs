use std::collections::HashSet;

/// Owns Skill-list selection independently from GPUI rendering state.
///
/// `primary` drives the detail pane, while `keys` is the complete action
/// selection. Keeping both here prevents list gestures, refreshes, and batch
/// actions from implementing slightly different reconciliation rules.
pub(super) struct SkillSelection {
    primary: Option<String>,
    anchor: Option<String>,
    multiple: bool,
    keys: HashSet<String>,
}

impl SkillSelection {
    pub(super) fn new(primary: Option<String>) -> Self {
        let keys = primary.iter().cloned().collect();
        Self {
            anchor: primary.clone(),
            primary,
            multiple: false,
            keys,
        }
    }

    pub(super) fn primary(&self) -> Option<&str> {
        self.primary.as_deref()
    }

    pub(super) fn set_primary(&mut self, primary: Option<String>) {
        self.primary = primary;
    }

    pub(super) fn is_multiple(&self) -> bool {
        self.multiple
    }

    pub(super) fn contains(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    pub(super) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(super) fn selected_in(&self, order: &[String]) -> Vec<String> {
        let selected = order
            .iter()
            .filter(|key| self.keys.contains(key.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            self.primary
                .as_ref()
                .filter(|primary| order.contains(primary))
                .cloned()
                .into_iter()
                .collect()
        } else {
            selected
        }
    }

    pub(super) fn reconcile(&mut self, order: &[String]) -> Option<String> {
        let existing = order.iter().collect::<HashSet<_>>();
        self.keys.retain(|key| existing.contains(key));
        if self.keys.is_empty()
            && let Some(primary) = self
                .primary
                .as_ref()
                .filter(|primary| existing.contains(*primary))
        {
            self.keys.insert(primary.clone());
        }
        if self
            .primary
            .as_ref()
            .is_none_or(|primary| !existing.contains(primary) || !self.keys.contains(primary))
        {
            self.primary = order
                .iter()
                .find(|key| self.keys.contains(key.as_str()))
                .or_else(|| order.first())
                .cloned();
        }
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !existing.contains(anchor))
        {
            self.anchor = self.primary.clone();
        }
        if self.keys.is_empty()
            && let Some(primary) = self.primary.clone()
        {
            self.keys.insert(primary.clone());
        }
        self.primary.clone()
    }

    pub(super) fn replace(&mut self, keys: &[String]) -> Option<String> {
        self.keys = keys.iter().cloned().collect();
        if self
            .primary
            .as_ref()
            .is_none_or(|primary| !self.keys.contains(primary))
        {
            self.primary = keys.first().cloned();
        }
        self.anchor = self.primary.clone();
        self.primary.clone()
    }

    pub(super) fn remove(&mut self, removed: &[String], order: &[String]) -> Option<String> {
        self.keys.retain(|key| !removed.contains(key));
        if self
            .primary
            .as_ref()
            .is_some_and(|primary| removed.contains(primary))
        {
            self.primary = None;
        }
        self.reconcile(order)
    }

    pub(super) fn finish(&mut self, order: &[String]) -> Option<String> {
        let primary = order
            .iter()
            .find(|key| self.keys.contains(key.as_str()))
            .or_else(|| {
                self.primary
                    .as_ref()
                    .filter(|primary| order.contains(primary))
            })
            .or_else(|| order.first())
            .cloned();
        self.multiple = false;
        self.keys = primary.iter().cloned().collect();
        self.anchor = primary.clone();
        self.primary = primary.clone();
        primary
    }

    pub(super) fn clear(&mut self) {
        self.multiple = false;
        self.keys.clear();
        self.anchor = None;
        self.primary = None;
    }

    pub(super) fn select_all(&mut self, visible: &[String]) -> Option<String> {
        self.multiple = true;
        self.keys = visible.iter().cloned().collect();
        if self
            .primary
            .as_ref()
            .is_none_or(|primary| !self.keys.contains(primary))
        {
            self.primary = visible.first().cloned();
        }
        self.anchor = self.primary.clone();
        self.primary.clone()
    }

    pub(super) fn toggle(&mut self, key: String, order: &[String]) -> Option<String> {
        self.multiple = true;
        if !self.keys.remove(&key) {
            self.keys.insert(key.clone());
        }
        if self.keys.is_empty() {
            self.primary = None;
        } else if self
            .primary
            .as_ref()
            .is_none_or(|primary| !self.keys.contains(primary))
        {
            self.primary = order
                .iter()
                .find(|candidate| self.keys.contains(candidate.as_str()))
                .cloned();
        }
        self.anchor = Some(key);
        self.primary.clone()
    }

    pub(super) fn select_range(&mut self, key: String, visible: &[String]) -> Option<String> {
        let anchor = self
            .anchor
            .as_ref()
            .or(self.primary.as_ref())
            .and_then(|anchor| visible.iter().position(|candidate| candidate == anchor));
        let target = visible.iter().position(|candidate| candidate == &key);
        let (Some(anchor), Some(target)) = (anchor, target) else {
            return self.select_one(key);
        };
        let (start, end) = if anchor <= target {
            (anchor, target)
        } else {
            (target, anchor)
        };
        self.multiple = true;
        self.keys = visible[start..=end].iter().cloned().collect();
        self.primary = Some(key);
        self.primary.clone()
    }

    pub(super) fn select_one(&mut self, key: String) -> Option<String> {
        self.multiple = false;
        self.keys.clear();
        self.keys.insert(key.clone());
        self.anchor = Some(key.clone());
        self.primary = Some(key);
        self.primary.clone()
    }

    pub(super) fn select_for_context(&mut self, key: String) -> Option<String> {
        if !self.keys.contains(&key) {
            self.multiple = false;
            self.keys.clear();
            self.keys.insert(key.clone());
        }
        self.primary = Some(key);
        self.primary.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::SkillSelection;

    fn keys(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn reconcile_drops_missing_keys_and_selects_first_survivor() {
        let mut selection = SkillSelection::new(Some("first".into()));
        selection.select_all(&keys(&["first", "second"]));

        assert_eq!(
            selection.reconcile(&keys(&["second", "third"])),
            Some("second".into())
        );
        assert_eq!(
            selection.selected_in(&keys(&["second", "third"])),
            keys(&["second"])
        );
    }

    #[test]
    fn toggle_keeps_detail_on_a_selected_skill() {
        let order = keys(&["first", "second", "third"]);
        let mut selection = SkillSelection::new(Some("first".into()));

        assert_eq!(
            selection.toggle("second".into(), &order),
            Some("first".into())
        );
        assert_eq!(
            selection.toggle("first".into(), &order),
            Some("second".into())
        );
        assert!(selection.is_multiple());
    }

    #[test]
    fn finishing_batch_selection_collapses_to_one_skill() {
        let order = keys(&["first", "second", "third"]);
        let mut selection = SkillSelection::new(Some("first".into()));
        selection.select_all(&keys(&["second", "third"]));

        assert_eq!(selection.finish(&order), Some("second".into()));
        assert_eq!(selection.len(), 1);
        assert!(!selection.is_multiple());
    }

    #[test]
    fn context_click_preserves_an_existing_batch() {
        let mut selection = SkillSelection::new(Some("first".into()));
        selection.select_all(&keys(&["first", "second"]));

        assert_eq!(
            selection.select_for_context("second".into()),
            Some("second".into())
        );
        assert_eq!(selection.len(), 2);
        assert!(selection.contains("first"));
    }

    #[test]
    fn shift_selection_uses_the_visible_order_in_both_directions() {
        let visible = keys(&["first", "third", "fifth", "seventh"]);
        let mut selection = SkillSelection::new(Some("third".into()));

        assert_eq!(
            selection.select_range("seventh".into(), &visible),
            Some("seventh".into())
        );
        assert_eq!(
            selection.selected_in(&visible),
            keys(&["third", "fifth", "seventh"])
        );

        assert_eq!(
            selection.select_range("first".into(), &visible),
            Some("first".into())
        );
        assert_eq!(selection.selected_in(&visible), keys(&["first", "third"]));
    }

    #[test]
    fn replacing_selection_resets_the_shift_anchor() {
        let visible = keys(&["first", "second", "third", "fourth"]);
        let mut selection = SkillSelection::new(Some("first".into()));

        selection.replace(&keys(&["third"]));
        selection.select_range("fourth".into(), &visible);

        assert_eq!(selection.selected_in(&visible), keys(&["third", "fourth"]));
    }
}
