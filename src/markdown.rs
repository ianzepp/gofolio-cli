use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// Render markdown text into styled, wrapped ratatui Lines.
pub fn render(input: &str, width: usize) -> Vec<Line<'static>> {
    let mut result = Vec::new();
    let lines: Vec<&str> = input.split('\n').collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Empty line
        if line.trim().is_empty() {
            result.push(Line::from(""));
            i += 1;
            continue;
        }

        // Headings
        if let Some(text) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
            .or_else(|| line.strip_prefix("# "))
        {
            result.push(Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(theme::AMBER)
                    .add_modifier(Modifier::BOLD),
            )));
            result.push(Line::from(""));
            i += 1;
            continue;
        }

        // Horizontal rule
        let trimmed = line.trim();
        if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_') {
            let rule = "\u{2500}".repeat(width.min(40));
            result.push(Line::from(Span::styled(
                rule,
                Style::default().fg(theme::MUTED),
            )));
            i += 1;
            continue;
        }

        // Fenced code block
        if line.starts_with("```") {
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                result.push(Line::from(Span::styled(
                    format!("  {}", lines[i]),
                    Style::default().fg(theme::MUTED),
                )));
                i += 1;
            }
            i += 1; // skip closing ```
            continue;
        }

        // Markdown pipe tables: consecutive lines starting with |
        if line.starts_with('|') {
            let mut table_lines = Vec::new();
            while i < lines.len() && lines[i].starts_with('|') {
                table_lines.push(lines[i]);
                i += 1;
            }
            result.extend(render_table_block(&table_lines));
            continue;
        }

        // Pre-formatted lines (box-drawing characters)
        if is_preformatted(line) {
            while i < lines.len() && is_preformatted(lines[i]) {
                result.push(Line::from(Span::raw(lines[i].to_string())));
                i += 1;
            }
            continue;
        }

        // Bullet list item
        if let Some((indent, text)) = parse_bullet(line) {
            let spans = parse_inline_spans(text);
            let mut bullet_spans = vec![Span::raw(format!("{indent}\u{2022} "))];
            bullet_spans.extend(spans);
            let bullet_line = Line::from(bullet_spans);
            result.extend(wrap_line(bullet_line, width));
            i += 1;
            continue;
        }

        // Plain text — collapse consecutive non-special lines into a paragraph
        let mut text_parts = Vec::new();
        while i < lines.len() && !is_block_start(lines[i]) {
            text_parts.push(lines[i]);
            i += 1;
        }
        let paragraph = text_parts.join(" ");
        let spans = parse_inline_spans(&paragraph);
        let para_line = Line::from(spans);
        result.extend(wrap_line(para_line, width));
    }

    result
}

/// Check if a line starts a block-level element.
fn is_block_start(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || line.starts_with('#')
        || line.starts_with('|')
        || line.starts_with("```")
        || is_preformatted(line)
        || parse_bullet(line).is_some()
        || (trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_'))
}

/// Check if a line contains box-drawing characters.
fn is_preformatted(line: &str) -> bool {
    line.contains('\u{2502}') // │
        || line.contains('\u{253C}') // ┼
        || line.contains('\u{2500}') // ─
        || line.contains('\u{2550}') // ═
        || line.contains('\u{2551}') // ║
}

/// Parse a bullet list item, returning (indent, text).
fn parse_bullet(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .map(|text| (indent, text))
}

/// Parse inline markdown (**bold**, *italic*, `code`) into styled Spans.
fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                // Flush current text
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }
                // Collect until closing `
                let mut code = String::new();
                while let Some(c2) = chars.next() {
                    if c2 == '`' {
                        break;
                    }
                    code.push(c2);
                }
                spans.push(Span::styled(code, Style::default().fg(theme::MUTED)));
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next(); // consume second *
                // Flush current text
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }
                // Collect until closing **
                let mut bold = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => {
                            chars.next();
                            break;
                        }
                        Some(c2) => bold.push(c2),
                        None => break,
                    }
                }
                spans.push(Span::styled(
                    bold,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }
            '*' => {
                // Single * = italic
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }
                let mut italic = String::new();
                while let Some(c2) = chars.next() {
                    if c2 == '*' {
                        break;
                    }
                    italic.push(c2);
                }
                spans.push(Span::styled(
                    italic,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
            }
            _ => current.push(c),
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

// --- Table rendering ---

fn render_table_block(lines: &[&str]) -> Vec<Line<'static>> {
    if lines.len() < 2 {
        return lines
            .iter()
            .map(|l| Line::from(Span::raw(l.to_string())))
            .collect();
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

    if !sep
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
    {
        return lines
            .iter()
            .map(|l| Line::from(Span::raw(l.to_string())))
            .collect();
    }

    let rows: Vec<Vec<String>> = lines[2..].iter().map(|l| split_row(l)).collect();

    let col_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut max = display_len(h);
            for row in &rows {
                if let Some(cell) = row.get(i) {
                    max = max.max(display_len(cell));
                }
            }
            max
        })
        .collect();

    let mut result = Vec::new();

    // Header
    result.push(Line::from(render_table_row(&headers, &col_widths, true)));

    // Separator
    let sep_str: String = col_widths
        .iter()
        .map(|w| "\u{2500}".repeat(*w))
        .collect::<Vec<_>>()
        .join("\u{2500}\u{253C}\u{2500}");
    result.push(Line::from(Span::styled(
        sep_str,
        Style::default().fg(theme::MUTED),
    )));

    // Data rows
    for row in &rows {
        result.push(Line::from(render_table_row(row, &col_widths, false)));
    }

    result
}

fn render_table_row(cells: &[String], col_widths: &[usize], bold: bool) -> Vec<Span<'static>> {
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

        let style = if bold {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(display, style));
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
    }
    spans
}

fn display_len(text: &str) -> usize {
    text.replace("**", "").len()
}

// --- Word wrapping ---

/// Wrap a single Line's spans to fit within `width` columns.
fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line];
    }

    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut col = 0;

    for span in line.spans {
        let style = span.style;
        let content = span.content.into_owned();

        if content.is_empty() {
            continue;
        }

        let words = split_words(&content);

        for word in words {
            let word_len = word.len();
            if word_len == 0 {
                continue;
            }

            // Hard-break words longer than width
            if word_len > width {
                if col > 0 {
                    rows.push(Vec::new());
                    col = 0;
                }
                let mut remaining = word.as_str();
                while !remaining.is_empty() {
                    let take = remaining.len().min(width);
                    let (chunk, rest) = remaining.split_at(take);
                    rows.last_mut()
                        .unwrap()
                        .push(Span::styled(chunk.to_string(), style));
                    remaining = rest;
                    if !remaining.is_empty() {
                        rows.push(Vec::new());
                        col = 0;
                    } else {
                        col = take;
                    }
                }
                continue;
            }

            // Word overflow — start new line
            if col > 0 && col + word_len > width {
                rows.push(Vec::new());
                col = 0;
                let trimmed = word.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                rows.last_mut()
                    .unwrap()
                    .push(Span::styled(trimmed.to_string(), style));
                col += trimmed.len();
            } else {
                rows.last_mut()
                    .unwrap()
                    .push(Span::styled(word, style));
                col += word_len;
            }
        }
    }

    rows.into_iter()
        .map(|spans| Line::from(spans).style(line.style))
        .collect()
}

/// Split text into word chunks, keeping whitespace attached to the following word.
fn split_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chars = text.chars().peekable();

    // First word
    let mut current = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        current.push(c);
        chars.next();
    }
    if !current.is_empty() {
        words.push(current);
    }

    // Subsequent: whitespace + word
    while chars.peek().is_some() {
        let mut current = String::new();
        while let Some(&c) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            current.push(c);
            chars.next();
        }
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            current.push(c);
            chars.next();
        }
        if !current.is_empty() {
            words.push(current);
        }
    }

    words
}

/// Apply a foreground color to all spans that don't already have one.
pub fn apply_default_color(lines: &mut [Line<'static>], color: ratatui::style::Color) {
    for line in lines.iter_mut() {
        for span in &mut line.spans {
            if span.style.fg.is_none() {
                span.style = span.style.fg(color);
            }
        }
    }
}
