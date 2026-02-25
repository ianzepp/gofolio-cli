use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Paragraph, Sparkline};
use ratatui::Frame;

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

        // Render markdown with line wrapping
        let mut msg_lines = markdown::render(&msg.text, content_width);

        // Apply default text color
        markdown::apply_default_color(&mut msg_lines, text_color);

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
    let mut total_rows: u16 = items.iter().map(|i| i.height()).sum();
    let mut start_idx = 0;
    while total_rows > total_height && start_idx < items.len() {
        total_rows -= items[start_idx].height();
        start_idx += 1;
    }

    // Render visible items top-down
    let mut y = area.y;
    for item in &items[start_idx..] {
        let h = item.height().min(area.y + area.height - y);
        if h == 0 {
            break;
        }
        let item_area = Rect::new(area.x + prefix_width as u16, y, area.width - prefix_width as u16, h);

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
}
