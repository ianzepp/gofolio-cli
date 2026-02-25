use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// A parsed block of markdown content.
pub enum Block {
    Heading { text: String },
    Hr,
    Table { headers: Vec<String>, rows: Vec<Vec<String>>, col_widths: Vec<usize> },
    Code { text: String },
    Text { text: String },
    Empty,
}

/// Parse markdown text into blocks.
pub fn parse_blocks(input: &str) -> Vec<Block> {
    let lines: Vec<&str> = input.split('\n').collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Fenced code block
        if line.starts_with("```") {
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            i += 1; // skip closing ```
            blocks.push(Block::Code {
                text: code_lines.join("\n"),
            });
            continue;
        }

        // Heading
        if let Some(rest) = line.strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
            .or_else(|| line.strip_prefix("# "))
        {
            blocks.push(Block::Heading {
                text: rest.to_string(),
            });
            i += 1;
            continue;
        }

        // Horizontal rule
        if line.len() >= 3
            && line
                .trim()
                .chars()
                .all(|c| c == '-' || c == '*' || c == '_')
        {
            blocks.push(Block::Hr);
            i += 1;
            continue;
        }

        // Table: consecutive lines starting with |
        if line.starts_with('|') {
            let mut table_lines = Vec::new();
            while i < lines.len() && lines[i].starts_with('|') {
                table_lines.push(lines[i]);
                i += 1;
            }
            if let Some(table) = parse_table(&table_lines) {
                blocks.push(table);
            } else {
                blocks.push(Block::Text {
                    text: table_lines.join("\n"),
                });
            }
            continue;
        }

        // Bullet
        if let Some(captures) = parse_bullet(line) {
            blocks.push(Block::Text {
                text: format!("{}\u{2022} {}", captures.0, captures.1),
            });
            i += 1;
            continue;
        }

        // Empty line
        if line.trim().is_empty() {
            blocks.push(Block::Empty);
            i += 1;
            continue;
        }

        // Plain text — collapse consecutive non-special lines
        let mut text_lines = Vec::new();
        while i < lines.len() && !is_special_line(lines[i]) {
            text_lines.push(lines[i]);
            i += 1;
        }
        blocks.push(Block::Text {
            text: text_lines.join(" "),
        });
    }

    // Collapse consecutive empties and remove empties adjacent to headings/hrs and remove empties adjacent to headings/hrs
    let mut collapsed = Vec::new();
    for block in blocks {
        let skip = match &block {
            Block::Empty => {
                matches!(collapsed.last(), None | Some(Block::Empty) | Some(Block::Hr | Block::Heading { .. }))
            }
            Block::Hr | Block::Heading { .. } => {
                // Remove preceding empty
                if matches!(collapsed.last(), Some(Block::Empty)) {
                    collapsed.pop();
                }
                false
            }
            _ => false,
        };
        if !skip {
            collapsed.push(block);
        }
    }

    collapsed
}

fn is_special_line(line: &str) -> bool {
    line.trim().is_empty()
        || line.starts_with('#')
        || line.starts_with('|')
        || line.starts_with("```")
        || (line.len() >= 3
            && line
                .trim()
                .chars()
                .all(|c| c == '-' || c == '*' || c == '_'))
        || parse_bullet(line).is_some()
}

fn parse_bullet(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];
    if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        Some((indent, rest))
    } else {
        None
    }
}

fn parse_table(lines: &[&str]) -> Option<Block> {
    if lines.len() < 2 {
        return None;
    }

    let split_row = |line: &str| -> Vec<String> {
        line.trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect()
    };

    let headers = split_row(lines[0]);
    let sep = split_row(lines[1]);

    // Validate separator
    if !sep.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
        return None;
    }

    let rows: Vec<Vec<String>> = lines[2..].iter().map(|l| split_row(l)).collect();

    // Calculate column widths
    let col_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut max = display_len(h);
            for row in &rows {
                if let Some(cell) = row.get(i) {
                    let len = display_len(cell);
                    if len > max {
                        max = len;
                    }
                }
            }
            max
        })
        .collect();

    Some(Block::Table {
        headers,
        rows,
        col_widths,
    })
}

/// Visible length after stripping **bold** markers.
fn display_len(text: &str) -> usize {
    text.replace("**", "").len()
}

/// Render a Block into ratatui Lines.
pub fn render_block(block: &Block, width: usize) -> Vec<Line<'static>> {
    match block {
        Block::Heading { text } => {
            vec![Line::from(Span::styled(
                text.clone(),
                Style::default().fg(theme::AMBER).add_modifier(Modifier::BOLD),
            ))]
        }
        Block::Hr => {
            let rule = "\u{2500}".repeat(width.min(40));
            vec![Line::from(Span::styled(rule, Style::default().fg(theme::MUTED)))]
        }
        Block::Code { text } => text
            .lines()
            .map(|l| {
                Line::from(Span::styled(
                    format!("  {l}"),
                    Style::default().fg(theme::MUTED),
                ))
            })
            .collect(),
        Block::Table {
            headers,
            rows,
            col_widths,
        } => render_table(headers, rows, col_widths),
        Block::Text { text } => {
            if text.is_empty() {
                return vec![Line::from("")];
            }
            vec![Line::from(parse_inline(text, theme::WHITE))]
        }
        Block::Empty => vec![Line::from("")],
    }
}

fn render_table(
    headers: &[String],
    rows: &[Vec<String>],
    col_widths: &[usize],
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header row
    let header_spans = render_table_row(headers, col_widths, theme::WHITE);
    lines.push(Line::from(header_spans));

    // Separator
    let sep: String = col_widths
        .iter()
        .map(|w| "\u{2500}".repeat(*w))
        .collect::<Vec<_>>()
        .join("\u{2500}\u{253C}\u{2500}");
    lines.push(Line::from(Span::styled(sep, Style::default().fg(theme::MUTED))));

    // Data rows
    for row in rows {
        let spans = render_table_row(row, col_widths, theme::WHITE);
        lines.push(Line::from(spans));
    }

    lines
}

fn render_table_row(
    cells: &[String],
    col_widths: &[usize],
    color: ratatui::style::Color,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " \u{2502} ".to_string(),
                Style::default().fg(theme::MUTED),
            ));
        }
        let w = col_widths.get(i).copied().unwrap_or(0);
        let display = cell.replace("**", "");
        let pad = w.saturating_sub(display.len());
        // Render with inline formatting
        let mut cell_spans = parse_inline(cell, color);
        if pad > 0 {
            cell_spans.push(Span::raw(" ".repeat(pad)));
        }
        spans.extend(cell_spans);
    }
    spans
}

/// Parse inline markdown (bold **text**) into styled Spans.
fn parse_inline(text: &str, color: ratatui::style::Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let parts: Vec<&str> = text.split("**").collect();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let style = if i % 2 == 1 {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        spans.push(Span::styled(part.to_string(), style));
    }
    spans
}
