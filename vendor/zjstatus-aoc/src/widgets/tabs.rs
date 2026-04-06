use std::{cmp, collections::BTreeMap};

use zellij_tile::{
    prelude::{InputMode, ModeInfo, PaneInfo, PaneManifest, TabInfo},
    shim::switch_tab_to,
};

use crate::{config::ZellijState, render::FormattedPart};

use super::widget::Widget;

pub struct TabsWidget {
    active_tab_format: Vec<FormattedPart>,
    active_tab_fullscreen_format: Vec<FormattedPart>,
    active_tab_sync_format: Vec<FormattedPart>,
    normal_tab_format: Vec<FormattedPart>,
    normal_tab_fullscreen_format: Vec<FormattedPart>,
    normal_tab_sync_format: Vec<FormattedPart>,
    rename_tab_format: Vec<FormattedPart>,
    separator: Option<FormattedPart>,
    fullscreen_indicator: Option<String>,
    floating_indicator: Option<String>,
    sync_indicator: Option<String>,
    tab_display_count: Option<usize>,
    tab_truncate_start_format: Vec<FormattedPart>,
    tab_truncate_end_format: Vec<FormattedPart>,
    config: BTreeMap<String, String>,
    project_segments: bool,
    project_palette: Vec<String>,
    project_group_left: String,
    project_group_right: String,
    project_same_separator: String,
    project_group_gap: String,
    project_active_fg: String,
    project_inactive_fg: String,
    project_separator_fg: String,
}

impl TabsWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut normal_tab_format: Vec<FormattedPart> = Vec::new();
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let normal_tab_fullscreen_format = match config.get("tab_normal_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let normal_tab_sync_format = match config.get("tab_normal_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let mut active_tab_format = normal_tab_format.clone();
        if let Some(form) = config.get("tab_active") {
            active_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let active_tab_fullscreen_format = match config.get("tab_active_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let active_tab_sync_format = match config.get("tab_active_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let rename_tab_format = match config.get("tab_rename") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let tab_display_count = match config.get("tab_display_count") {
            Some(count) => count.parse::<usize>().ok(),
            None => None,
        };

        let tab_truncate_start_format = config
            .get("tab_truncate_start_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let tab_truncate_end_format = config
            .get("tab_truncate_end_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let separator = config
            .get("tab_separator")
            .map(|s| FormattedPart::from_format_string(s, config));

        let project_palette = config
            .get("tab_project_palette")
            .map(|palette| {
                palette
                    .split(',')
                    .map(|color| color.trim().to_owned())
                    .filter(|color| !color.is_empty())
                    .collect::<Vec<String>>()
            })
            .filter(|palette| !palette.is_empty())
            .unwrap_or_else(|| {
                vec![
                    "#4A9EFF".to_owned(),
                    "#CC6EFF".to_owned(),
                    "#00E1FF".to_owned(),
                    "#FF9C52".to_owned(),
                    "#00F5A0".to_owned(),
                    "#FFCE56".to_owned(),
                ]
            });

        Self {
            normal_tab_format,
            normal_tab_fullscreen_format,
            normal_tab_sync_format,
            active_tab_format,
            active_tab_fullscreen_format,
            active_tab_sync_format,
            rename_tab_format,
            separator,
            floating_indicator: config.get("tab_floating_indicator").cloned(),
            sync_indicator: config.get("tab_sync_indicator").cloned(),
            fullscreen_indicator: config.get("tab_fullscreen_indicator").cloned(),
            tab_display_count,
            tab_truncate_start_format,
            tab_truncate_end_format,
            config: config.clone(),
            project_segments: config
                .get("tab_project_segments")
                .map(|toggle| toggle == "true")
                .unwrap_or(false),
            project_palette,
            project_group_left: config
                .get("tab_project_group_left")
                .cloned()
                .unwrap_or_else(|| "".to_owned()),
            project_group_right: config
                .get("tab_project_group_right")
                .cloned()
                .unwrap_or_else(|| "".to_owned()),
            project_same_separator: config
                .get("tab_project_same_separator")
                .cloned()
                .unwrap_or_else(|| "".to_owned()),
            project_group_gap: config
                .get("tab_project_group_gap")
                .cloned()
                .unwrap_or_else(|| "  ".to_owned()),
            project_active_fg: config
                .get("tab_project_active_fg")
                .cloned()
                .unwrap_or_else(|| "#111827".to_owned()),
            project_inactive_fg: config
                .get("tab_project_inactive_fg")
                .cloned()
                .unwrap_or_else(|| "#111827".to_owned()),
            project_separator_fg: config
                .get("tab_project_separator_fg")
                .cloned()
                .unwrap_or_else(|| "#111827".to_owned()),
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        if self.project_segments {
            return self.process_project_segments(state);
        }

        let mut output = "".to_owned();
        let mut counter = 0;

        let (truncated_start, truncated_end, tabs) =
            get_tab_window(&state.tabs, self.tab_display_count);

        if truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content = content.replace("{count}", (truncated_start).to_string().as_str());
                }

                output = format!("{output}{}", f.format_string(&content));
            }
        }

        for tab in &tabs {
            let content = self.render_tab(tab, &state.panes, &state.mode);
            counter += 1;

            output = format!("{}{}", output, content);

            if counter < tabs.len()
                && let Some(sep) = &self.separator
            {
                output = format!("{}{}", output, sep.format_string(&sep.content));
            }
        }

        if truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content = content.replace("{count}", (truncated_end).to_string().as_str());
                }

                output = format!("{output}{}", f.format_string(&content));
            }
        }

        output
    }

    fn process_click(&self, _name: &str, state: &ZellijState, pos: usize) {
        if self.project_segments {
            self.process_project_segment_click(state, pos);
            return;
        }

        let mut offset = 0;
        let mut counter = 0;

        let (truncated_start, truncated_end, tabs) =
            get_tab_window(&state.tabs, self.tab_display_count);

        let active_pos = &state
            .tabs
            .iter()
            .find(|t| t.active)
            .expect("no active tab")
            .position
            + 1;

        if truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content = content.replace("{count}", (truncated_start).to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));

                if pos <= offset {
                    switch_tab_to(active_pos.saturating_sub(1) as u32);
                }
            }
        }

        for tab in &tabs {
            counter += 1;

            let mut rendered_content = self.render_tab(tab, &state.panes, &state.mode);

            if counter < tabs.len()
                && let Some(sep) = &self.separator
            {
                rendered_content =
                    format!("{}{}", rendered_content, sep.format_string(&sep.content));
            }

            let content_len = console::measure_text_width(&rendered_content);

            if pos > offset && pos < offset + content_len {
                switch_tab_to(tab.position as u32 + 1);

                break;
            }

            offset += content_len;
        }

        if truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content = content.replace("{count}", (truncated_end).to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));

                if pos <= offset {
                    switch_tab_to(cmp::min(active_pos + 1, state.tabs.len()) as u32);
                }
            }
        }
    }
}

impl TabsWidget {
    fn process_project_segments(&self, state: &ZellijState) -> String {
        let mut output = String::new();
        let (truncated_start, truncated_end, tabs) =
            get_tab_window(&state.tabs, self.tab_display_count);
        let project_palette = self.effective_project_palette(state);
        let active_fg = self.effective_active_fg(state);
        let project_colors = self.project_color_map(&tabs, &project_palette);

        if truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();
                if content.contains("{count}") {
                    content = content.replace("{count}", truncated_start.to_string().as_str());
                }
                output.push_str(&f.format_string(&content));
            }
        }

        for (index, tab) in tabs.iter().enumerate() {
            let prev = index
                .checked_sub(1)
                .and_then(|prev_index| tabs.get(prev_index));
            let next = tabs.get(index + 1);
            output.push_str(&self.render_project_segment_tab(
                tab,
                prev,
                next,
                &project_colors,
                active_fg.as_str(),
                &state.panes,
                &state.mode,
            ));
        }

        if truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();
                if content.contains("{count}") {
                    content = content.replace("{count}", truncated_end.to_string().as_str());
                }
                output.push_str(&f.format_string(&content));
            }
        }

        output
    }

    fn process_project_segment_click(&self, state: &ZellijState, pos: usize) {
        let mut offset = 0;
        let (truncated_start, truncated_end, tabs) =
            get_tab_window(&state.tabs, self.tab_display_count);
        let project_palette = self.effective_project_palette(state);
        let active_fg = self.effective_active_fg(state);
        let project_colors = self.project_color_map(&tabs, &project_palette);

        let active_pos = &state
            .tabs
            .iter()
            .find(|t| t.active)
            .expect("no active tab")
            .position
            + 1;

        if truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();
                if content.contains("{count}") {
                    content = content.replace("{count}", truncated_start.to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));
                if pos <= offset {
                    switch_tab_to(active_pos.saturating_sub(1) as u32);
                    return;
                }
            }
        }

        for (index, tab) in tabs.iter().enumerate() {
            let prev = index
                .checked_sub(1)
                .and_then(|prev_index| tabs.get(prev_index));
            let next = tabs.get(index + 1);
            let rendered_content = self.render_project_segment_tab(
                tab,
                prev,
                next,
                &project_colors,
                active_fg.as_str(),
                &state.panes,
                &state.mode,
            );
            let content_len = console::measure_text_width(&rendered_content);

            if pos > offset && pos < offset + content_len {
                switch_tab_to(tab.position as u32 + 1);
                return;
            }

            offset += content_len;
        }

        if truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();
                if content.contains("{count}") {
                    content = content.replace("{count}", truncated_end.to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));
                if pos <= offset {
                    switch_tab_to(cmp::min(active_pos + 1, state.tabs.len()) as u32);
                    return;
                }
            }
        }
    }

    fn select_format(&self, info: &TabInfo, mode: &ModeInfo) -> &Vec<FormattedPart> {
        if info.active && mode.mode == InputMode::RenameTab {
            return &self.rename_tab_format;
        }

        if info.active && info.is_fullscreen_active {
            return &self.active_tab_fullscreen_format;
        }

        if info.active && info.is_sync_panes_active {
            return &self.active_tab_sync_format;
        }

        if info.active {
            return &self.active_tab_format;
        }

        if info.is_fullscreen_active {
            return &self.normal_tab_fullscreen_format;
        }

        if info.is_sync_panes_active {
            return &self.normal_tab_sync_format;
        }

        &self.normal_tab_format
    }

    fn render_project_segment_tab(
        &self,
        tab: &TabInfo,
        prev: Option<&TabInfo>,
        next: Option<&TabInfo>,
        project_colors: &BTreeMap<String, String>,
        active_fg: &str,
        panes: &PaneManifest,
        mode: &ModeInfo,
    ) -> String {
        let project_key = self.project_key(tab);
        let prev_same_project = prev
            .map(|prev_tab| self.project_key(prev_tab) == project_key)
            .unwrap_or(false);
        let next_same_project = next
            .map(|next_tab| self.project_key(next_tab) == project_key)
            .unwrap_or(false);
        let project_color = project_colors
            .get(&project_key)
            .cloned()
            .unwrap_or_else(|| self.project_palette[0].clone());
        let label = format!(" {} ", self.render_project_label(tab, panes, mode));

        let mut output = String::new();

        if !prev_same_project {
            output.push_str(&self.style_fragment(
                Some(project_color.as_str()),
                None,
                &[],
                &self.project_group_left,
            ));
        }

        let inactive_text_color = self
            .darken_hex_color(&project_color, 0.58)
            .unwrap_or_else(|| self.project_inactive_fg.clone());
        let separator_color = self
            .darken_hex_color(&project_color, 0.68)
            .unwrap_or_else(|| self.project_separator_fg.clone());

        let text_effects: &[&str] = if tab.active { &["bold", "italic"] } else { &[] };
        let text_color = if tab.active {
            active_fg
        } else {
            inactive_text_color.as_str()
        };

        output.push_str(&self.style_fragment(
            Some(text_color),
            Some(project_color.as_str()),
            text_effects,
            &label,
        ));

        if next_same_project {
            output.push_str(&self.style_fragment(
                Some(separator_color.as_str()),
                Some(project_color.as_str()),
                &[],
                &self.project_same_separator,
            ));
        } else {
            output.push_str(&self.style_fragment(
                Some(project_color.as_str()),
                None,
                &[],
                &self.project_group_right,
            ));
            if next.is_some() {
                output.push_str(&self.project_group_gap);
            }
        }

        output
    }

    fn render_project_label(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo) -> String {
        let mut label = self.resolved_tab_name(tab, mode).to_owned();
        let indicators = self.tab_indicator_suffix(tab, panes);
        if !indicators.is_empty() {
            label.push(' ');
            label.push_str(&indicators);
        }
        label
    }

    fn resolved_tab_name<'a>(&self, tab: &'a TabInfo, mode: &ModeInfo) -> &'a str {
        match mode.mode {
            InputMode::RenameTab => match tab.name.is_empty() {
                true => "Enter name...",
                false => tab.name.as_str(),
            },
            _ => tab.name.as_str(),
        }
    }

    fn tab_indicator_suffix(&self, tab: &TabInfo, panes: &PaneManifest) -> String {
        let mut indicators = Vec::new();

        if tab.is_fullscreen_active
            && let Some(indicator) = &self.fullscreen_indicator
            && !indicator.is_empty()
        {
            indicators.push(indicator.clone());
        }

        if tab.is_sync_panes_active
            && let Some(indicator) = &self.sync_indicator
            && !indicator.is_empty()
        {
            indicators.push(indicator.clone());
        }

        if let Some(indicator) = &self.floating_indicator
            && !indicator.is_empty()
        {
            let panes_for_tab: Vec<PaneInfo> =
                panes.panes.get(&tab.position).cloned().unwrap_or_default();
            if panes_for_tab.iter().any(|pane| pane.is_floating) {
                indicators.push(indicator.clone());
            }
        }

        indicators.join(" ")
    }

    fn project_key(&self, tab: &TabInfo) -> String {
        let trimmed = tab.name.trim();
        let token = trimmed
            .split(|c: char| {
                c.is_whitespace()
                    || matches!(
                        c,
                        '/' | '\\' | '|' | ':' | '[' | ']' | '(' | ')' | '<' | '>' | '{' | '}'
                    )
            })
            .find(|part| !part.is_empty())
            .unwrap_or(trimmed);

        let normalized = token
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
            .to_ascii_lowercase();

        if normalized.is_empty() {
            format!("tab-{}", tab.position)
        } else {
            normalized
        }
    }

    fn project_color_map(
        &self,
        tabs: &[TabInfo],
        project_palette: &[String],
    ) -> BTreeMap<String, String> {
        let mut project_colors = BTreeMap::new();
        let mut next_index = 0usize;

        for tab in tabs {
            let key = self.project_key(tab);
            if project_colors.contains_key(&key) {
                continue;
            }

            let color = project_palette[next_index % project_palette.len()].clone();
            project_colors.insert(key, color);
            next_index += 1;
        }

        project_colors
    }

    fn effective_project_palette(&self, state: &ZellijState) -> Vec<String> {
        let runtime_palette = state.runtime_theme.project_palette();
        if runtime_palette.is_empty() {
            return self.project_palette.clone();
        }
        runtime_palette
    }

    fn effective_active_fg(&self, state: &ZellijState) -> String {
        if !state.runtime_theme.bg_base.is_empty() {
            return state.runtime_theme.bg_base.clone();
        }
        self.project_active_fg.clone()
    }

    fn darken_hex_color(&self, color: &str, factor: f32) -> Option<String> {
        let hex = color.trim();
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        let scale = factor.clamp(0.0, 1.0);
        let darken = |value: u8| -> u8 { ((value as f32) * scale).round() as u8 };

        Some(format!(
            "#{:02X}{:02X}{:02X}",
            darken(r),
            darken(g),
            darken(b)
        ))
    }

    fn style_fragment(
        &self,
        fg: Option<&str>,
        bg: Option<&str>,
        effects: &[&str],
        content: &str,
    ) -> String {
        let mut attrs = Vec::new();
        if let Some(fg) = fg {
            attrs.push(format!("fg={fg}"));
        }
        if let Some(bg) = bg {
            attrs.push(format!("bg={bg}"));
        }
        attrs.extend(effects.iter().map(|effect| effect.to_string()));

        let styled_content = if attrs.is_empty() {
            content.to_owned()
        } else {
            format!("#[{}]{}", attrs.join(","), content)
        };

        FormattedPart::multiple_from_format_string(&styled_content, &self.config)
            .iter()
            .map(|part| part.format_string(&part.content))
            .collect::<Vec<String>>()
            .join("")
    }

    fn render_tab(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo) -> String {
        let formatters = self.select_format(tab, mode);
        let mut output = "".to_owned();

        for f in formatters.iter() {
            let mut content = f.content.clone();

            let tab_name = self.resolved_tab_name(tab, mode);

            if content.contains("{name}") {
                content = content.replace("{name}", tab_name);
            }

            if content.contains("{index}") {
                content = content.replace("{index}", (tab.position + 1).to_string().as_str());
            }

            if content.contains("{floating_total_count}") {
                let panes_for_tab: Vec<PaneInfo> =
                    panes.panes.get(&tab.position).cloned().unwrap_or_default();

                content = content.replace(
                    "{floating_total_count}",
                    &format!("{}", panes_for_tab.iter().filter(|p| p.is_floating).count()),
                );
            }

            content = self.replace_indicators(content, tab, panes);

            output = format!("{}{}", output, f.format_string(&content));
        }

        output.to_owned()
    }

    fn replace_indicators(&self, content: String, tab: &TabInfo, panes: &PaneManifest) -> String {
        let mut content = content;
        if content.contains("{fullscreen_indicator}") && self.fullscreen_indicator.is_some() {
            content = content.replace(
                "{fullscreen_indicator}",
                if tab.is_fullscreen_active {
                    self.fullscreen_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        if content.contains("{sync_indicator}") && self.sync_indicator.is_some() {
            content = content.replace(
                "{sync_indicator}",
                if tab.is_sync_panes_active {
                    self.sync_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        if content.contains("{floating_indicator}") && self.floating_indicator.is_some() {
            let panes_for_tab: Vec<PaneInfo> =
                panes.panes.get(&tab.position).cloned().unwrap_or_default();

            let is_floating = panes_for_tab.iter().any(|p| p.is_floating);

            content = content.replace(
                "{floating_indicator}",
                if is_floating {
                    self.floating_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        content
    }
}

pub fn get_tab_window(
    tabs: &Vec<TabInfo>,
    max_count: Option<usize>,
) -> (usize, usize, Vec<TabInfo>) {
    let max_count = match max_count {
        Some(count) => count,
        None => return (0, 0, tabs.to_vec()),
    };

    if tabs.len() <= max_count {
        return (0, 0, tabs.to_vec());
    }

    let active_index = tabs.iter().position(|t| t.active).expect("no active tab");

    // active tab is in the last #max_count tabs, so return the last #max_count
    if active_index > tabs.len().saturating_sub(max_count) {
        return (
            tabs.len().saturating_sub(max_count),
            0,
            tabs.iter()
                .cloned()
                .rev()
                .take(max_count)
                .rev()
                .collect::<Vec<TabInfo>>(),
        );
    }

    // tabs must be truncated
    let first_index = active_index.saturating_sub(1);
    let last_index = cmp::min(first_index + max_count, tabs.len());

    (
        first_index,
        tabs.len().saturating_sub(last_index),
        tabs.as_slice()[first_index..last_index].to_vec(),
    )
}

#[cfg(test)]
mod test {
    use zellij_tile::prelude::TabInfo;

    use super::get_tab_window;
    use rstest::rstest;

    #[rstest]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (1, 1, vec![
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: true,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: true,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: false,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        None,
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    pub fn test_get_tab_window(
        #[case] tabs: Vec<TabInfo>,
        #[case] max_count: Option<usize>,
        #[case] expected: (usize, usize, Vec<TabInfo>),
    ) {
        let res = get_tab_window(&tabs, max_count);

        assert_eq!(res, expected);
    }
}
