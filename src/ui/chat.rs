use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Paragraph, Sparkline};
use std::sync::OnceLock;

use crate::app::{AppState, ChartData};
use crate::markdown;
use crate::theme;

/// A renderable item in the chat panel — either text lines or an inline chart.
enum ChatItem {
    Lines(Vec<Line<'static>>),
    SparklineChart {
        title: String,
        data: Vec<u64>,
    },
    BarChartData {
        title: String,
        labels: Vec<String>,
        values: Vec<u64>,
    },
}

impl ChatItem {
    /// How many terminal rows this item needs.
    fn height(&self) -> u16 {
        match self {
            ChatItem::Lines(lines) => lines.len() as u16,
            ChatItem::SparklineChart { .. } => 5, // 3 rows chart + 2 border
            ChatItem::BarChartData { values, .. } => {
                // Height = bar area + label + border. Cap at 10.
                let bar_height = if values.is_empty() { 1 } else { 7 };
                (bar_height + 2).min(10)
            }
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let prefix_width = 4; // "YOU " / "AGT " / "SYS " / "    "
    let content_width = (area.width as usize).saturating_sub(prefix_width + 2);
    let total_height = area.height;

    // Build all chat items
    let mut items: Vec<ChatItem> = Vec::new();

    for msg in &state.messages {
        // Chart message — render as widget
        if let Some(chart) = &msg.chart {
            match chart {
                ChartData::Sparkline { title, data } => {
                    items.push(ChatItem::SparklineChart {
                        title: title.clone(),
                        data: data.clone(),
                    });
                }
                ChartData::Bar {
                    title,
                    labels,
                    values,
                } => {
                    items.push(ChatItem::BarChartData {
                        title: title.clone(),
                        labels: labels.clone(),
                        values: values.clone(),
                    });
                }
            }
            continue;
        }

        let (label, label_color) = match msg.role.as_str() {
            "user" => ("YOU", theme::AMBER),
            "agent" => ("AGT", theme::AMBER),
            _ => ("SYS", theme::AMBER),
        };

        let text_color = if msg.is_warning {
            theme::WARNING
        } else {
            theme::WHITE
        };

        // Render agent messages with markdown by default, unless disabled
        // for debugging via env var.
        let mut msg_lines: Vec<Line<'static>> = if msg.role == "agent" {
            if markdown_disabled() {
                msg.text
                    .lines()
                    .flat_map(|line| wrap_line(line, content_width, text_color))
                    .collect()
            } else {
                let mut lines = markdown::render(&msg.text, content_width);
                markdown::apply_default_color(&mut lines, text_color);
                lines
            }
        } else {
            msg.text
                .lines()
                .flat_map(|line| wrap_line(line, content_width, text_color))
                .collect()
        };

        if msg_lines.is_empty() {
            msg_lines.push(Line::from(Span::styled(
                "",
                Style::default().fg(text_color),
            )));
        }

        if let Some(footer) = &msg.footer {
            let footer_color = if msg.footer_is_warning {
                theme::WARNING
            } else {
                theme::MUTED
            };
            msg_lines.extend(
                footer
                    .lines()
                    .flat_map(|line| wrap_line(line, content_width, footer_color)),
            );
        }

        normalize_lines(&mut msg_lines);

        // Prepend role label to first line, indent continuation lines
        for (i, line) in msg_lines.iter_mut().enumerate() {
            let prefix = if i == 0 {
                Span::styled(
                    format!("{label} "),
                    Style::default()
                        .fg(label_color)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("    ")
            };

            let mut new_spans = vec![prefix];
            new_spans.append(&mut line.spans);
            *line = Line::from(new_spans);
        }

        items.push(ChatItem::Lines(msg_lines));
    }

    // Spinner when loading
    if state.loading {
        items.push(ChatItem::Lines(vec![Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("\u{25CF} ", Style::default().fg(theme::AMBER)),
            Span::styled("Thinking...", Style::default().fg(theme::AMBER)),
        ])]));
    }

    // Scroll to bottom: figure out which items fit in the viewport
    // First, find the default start_idx (pinned to bottom)
    let all_rows: u16 = items.iter().map(|i| i.height()).sum();
    let mut bottom_start_idx = 0;
    {
        let mut rows = all_rows;
        while rows > total_height && bottom_start_idx < items.len() {
            rows -= items[bottom_start_idx].height();
            bottom_start_idx += 1;
        }
    }

    // Apply scroll offset: move start_idx backwards (towards top) by scroll_offset rows
    let mut start_idx = bottom_start_idx;
    let mut remaining_scroll = state.scroll_offset;
    while remaining_scroll > 0 && start_idx > 0 {
        start_idx -= 1;
        let h = items[start_idx].height();
        if h > remaining_scroll {
            remaining_scroll = 0;
        } else {
            remaining_scroll -= h;
        }
    }

    // Figure out which items fit from start_idx forward
    let mut end_idx = start_idx;
    let mut visible_rows: u16 = 0;
    while end_idx < items.len() && visible_rows + items[end_idx].height() <= total_height {
        visible_rows += items[end_idx].height();
        end_idx += 1;
    }

    // Render visible items top-down
    let mut y = area.y;
    for item in &items[start_idx..end_idx] {
        let h = item.height().min(area.y + area.height - y);
        if h == 0 {
            break;
        }
        let item_area = Rect::new(
            area.x + prefix_width as u16,
            y,
            area.width - prefix_width as u16,
            h,
        );

        match item {
            ChatItem::Lines(lines) => {
                let para = Paragraph::new(lines.clone());
                // Lines render at area.x (they already have prefix baked in)
                let full_area = Rect::new(area.x, y, area.width, h);
                frame.render_widget(para, full_area);
            }
            ChatItem::SparklineChart { title, data } => {
                let sparkline = Sparkline::default()
                    .block(
                        Block::bordered()
                            .title(title.as_str())
                            .title_style(Style::default().fg(theme::AMBER).bold())
                            .border_style(Style::default().fg(theme::BORDER)),
                    )
                    .data(data)
                    .style(Style::default().fg(theme::GREEN));
                frame.render_widget(sparkline, item_area);
            }
            ChatItem::BarChartData {
                title,
                labels,
                values,
            } => {
                let bars: Vec<Bar> = labels
                    .iter()
                    .zip(values.iter())
                    .map(|(label, &val)| {
                        Bar::default()
                            .label(Line::from(label.as_str()))
                            .value(val)
                            .style(Style::default().fg(theme::AMBER))
                    })
                    .collect();

                let max_label_len = labels.iter().map(|l| l.len()).max().unwrap_or(3);
                let bar_width = (max_label_len as u16).clamp(3, 8);

                let chart = BarChart::default()
                    .block(
                        Block::bordered()
                            .title(title.as_str())
                            .title_style(Style::default().fg(theme::AMBER).bold())
                            .border_style(Style::default().fg(theme::BORDER)),
                    )
                    .data(BarGroup::default().bars(&bars))
                    .bar_width(bar_width)
                    .bar_gap(1)
                    .bar_style(Style::default().fg(theme::AMBER))
                    .value_style(Style::default().fg(theme::WHITE).bold())
                    .label_style(Style::default().fg(theme::MUTED));
                frame.render_widget(chart, item_area);
            }
        }

        y += h;
    }

    // Scroll indicator when not at bottom
    if state.scroll_offset > 0 && area.height > 0 {
        let indicator = Line::from(vec![Span::styled(
            " \u{2193} more below (End to jump) ",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        )]);
        let indicator_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
        frame.render_widget(Paragraph::new(vec![indicator]), indicator_area);
    }
}

fn markdown_disabled() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("GHOSTFOLIO_DISABLE_MARKDOWN")
            .ok()
            .or_else(|| std::env::var("GF_DISABLE_MARKDOWN").ok())
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
    })
}

fn wrap_line(line: &str, max_width: usize, color: ratatui::style::Color) -> Vec<Line<'static>> {
    let line = sanitize_line(line);
    if max_width == 0 || line.is_empty() {
        return vec![Line::from(Span::styled(
            line,
            Style::default().fg(color),
        ))];
    }

    // Deterministic Unicode-safe wrapping: split by character count only.
    // This avoids byte-boundary issues and avoids whitespace-trimming artifacts.
    let mut out = Vec::new();
    let mut remaining = line;
    while !remaining.is_empty() {
        let chunk = take_prefix_chars(&remaining, max_width);
        let consumed = chunk.chars().count();
        // Drop whitespace-only chunks to prevent large blank regions from
        // deeply-indented/ASCII-art lines.
        if !chunk.trim().is_empty() {
            out.push(Line::from(Span::styled(chunk, Style::default().fg(color))));
        }
        if consumed == 0 {
            break;
        }
        remaining = drop_prefix_chars(&remaining, consumed);
    }
    if out.is_empty() {
        out.push(Line::from(Span::styled("", Style::default().fg(color))));
    }
    out
}

fn take_prefix_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 || s.is_empty() {
        return String::new();
    }
    s.chars().take(max_chars).collect()
}

fn drop_prefix_chars(s: &str, count: usize) -> String {
    if count == 0 || s.is_empty() {
        return s.to_string();
    }
    s.chars().skip(count).collect()
}

fn sanitize_line(line: &str) -> String {
    line.chars()
        .filter(|&c| c == '\t' || !c.is_control())
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn normalize_lines(lines: &mut Vec<Line<'static>>) {
    if lines.is_empty() {
        return;
    }

    let mut normalized = Vec::with_capacity(lines.len());
    for line in lines.drain(..) {
        let blank = line.spans.is_empty()
            || (line.spans.len() == 1 && line.spans[0].content.trim().is_empty());
        let prev_blank = normalized.last().is_some_and(|l: &Line<'static>| {
            l.spans.is_empty() || (l.spans.len() == 1 && l.spans[0].content.trim().is_empty())
        });
        if blank && prev_blank {
            continue;
        }
        normalized.push(line);
    }

    while normalized
        .first()
        .is_some_and(|l| l.spans.is_empty() || (l.spans.len() == 1 && l.spans[0].content.trim().is_empty()))
    {
        normalized.remove(0);
    }
    while normalized
        .last()
        .is_some_and(|l| l.spans.is_empty() || (l.spans.len() == 1 && l.spans[0].content.trim().is_empty()))
    {
        normalized.pop();
    }

    if normalized.is_empty() {
        normalized.push(Line::from(Span::raw("")));
    }

    *lines = normalized;
}
