//! Overview/tab support helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn seed_tab_cache(rows: &[OverviewRow]) -> HashMap<String, TabMeta> {
    let mut cache = HashMap::new();
    merge_tab_cache(&mut cache, rows);
    cache
}

pub(crate) fn merge_tab_cache(cache: &mut HashMap<String, TabMeta>, rows: &[OverviewRow]) {
    for row in rows {
        let Some(index) = row.tab_index else {
            continue;
        };
        let name = row
            .tab_name
            .clone()
            .unwrap_or_else(|| format!("tab-{index}"));
        cache.insert(
            row.pane_id.clone(),
            TabMeta {
                index,
                name,
                focused: row.tab_focused,
            },
        );
    }
}

pub(crate) fn apply_cached_tab_meta(row: &mut OverviewRow, cache: &HashMap<String, TabMeta>) {
    let Some(cached) = cache.get(&row.pane_id) else {
        return;
    };
    if row.tab_index.is_none() {
        row.tab_index = Some(cached.index);
    }
    if row.tab_name.is_none() {
        row.tab_name = Some(cached.name.clone());
    }
    if !row.tab_focused && cached.focused {
        row.tab_focused = true;
    }
}

pub(crate) fn normalized_project_root_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut normalized = trimmed.replace('\\', "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

pub(crate) fn pane_id_number(pane_id: &str) -> Option<u64> {
    pane_id.trim().parse::<u64>().ok()
}

pub(crate) fn overview_layout_cmp(left: &OverviewRow, right: &OverviewRow) -> std::cmp::Ordering {
    left.tab_index
        .is_none()
        .cmp(&right.tab_index.is_none())
        .then_with(|| {
            left.tab_index
                .unwrap_or(usize::MAX)
                .cmp(&right.tab_index.unwrap_or(usize::MAX))
        })
        .then_with(|| {
            let left_pane = pane_id_number(&left.pane_id);
            let right_pane = pane_id_number(&right.pane_id);
            left_pane
                .is_none()
                .cmp(&right_pane.is_none())
                .then_with(|| {
                    left_pane
                        .unwrap_or(u64::MAX)
                        .cmp(&right_pane.unwrap_or(u64::MAX))
                })
                .then_with(|| left.pane_id.cmp(&right.pane_id))
        })
        .then_with(|| left.identity_key.cmp(&right.identity_key))
}

pub(crate) fn sort_overview_rows(mut rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
    rows.sort_by(overview_layout_cmp);
    rows
}

pub(crate) fn sort_overview_rows_attention(mut rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
    rows.sort_by(|left, right| {
        attention_chip_from_row(left)
            .severity()
            .cmp(&attention_chip_from_row(right).severity())
            .then_with(|| right.tab_focused.cmp(&left.tab_focused))
            .then_with(|| overview_layout_cmp(left, right))
    });
    rows
}
