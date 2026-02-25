use ratatui::text::{Line, Span};

/// Render markdown text into wrapped ratatui Lines.
///
/// Uses `tui-markdown` (pulldown-cmark) for parsing, then wraps long lines
/// to fit within `width` columns.
pub fn render(input: &str, width: usize) -> Vec<Line<'static>> {
    let text = tui_markdown::from_str(input);
    let mut result = Vec::new();

    for line in text.lines {
        // Convert borrowed spans to owned so we can return 'static
        let owned_line = own_line(line);
        let wrapped = wrap_line(owned_line, width);
        result.extend(wrapped);
    }

    result
}

/// Convert a Line with borrowed content into one with owned (static) content.
fn own_line(line: Line<'_>) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| Span::styled(s.content.into_owned(), s.style))
        .collect();
    Line::from(spans).style(line.style)
}

/// Wrap a single Line's spans to fit within `width` columns.
///
/// Walks through each span, splitting on word boundaries when the current
/// row exceeds the width. Continuation lines are not indented.
fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line];
    }

    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut col = 0;

    for span in line.spans {
        let style = span.style;
        let content = span.content.into_owned();

        // For spans that are entirely whitespace or empty, just append
        if content.is_empty() {
            continue;
        }

        // Split content into words, preserving whitespace
        let mut words = split_words(&content);

        for word in words.drain(..) {
            let word_len = word.len();

            if word_len == 0 {
                continue;
            }

            // If this word alone exceeds width, force it on a new line and hard-break
            if word_len > width {
                if col > 0 {
                    rows.push(Vec::new());
                    col = 0;
                }
                // Hard-break the long word
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

            // Would this word overflow the current line?
            if col > 0 && col + word_len > width {
                // Start a new line
                rows.push(Vec::new());
                col = 0;

                // Skip leading whitespace on the new line
                let trimmed = word.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                let trimmed_len = trimmed.len();
                rows.last_mut()
                    .unwrap()
                    .push(Span::styled(trimmed.to_string(), style));
                col += trimmed_len;
            } else {
                rows.last_mut()
                    .unwrap()
                    .push(Span::styled(word.clone(), style));
                col += word_len;
            }
        }
    }

    rows.into_iter()
        .map(|spans| Line::from(spans).style(line.style))
        .collect()
}

/// Split text into word chunks, keeping whitespace attached to the following word
/// so that wrapping doesn't lose spaces.
///
/// Example: "hello world  foo" → ["hello", " world", "  foo"]
fn split_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chars = text.chars().peekable();

    // First word: no leading whitespace grouping
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

    // Subsequent words: whitespace prefix + word
    while chars.peek().is_some() {
        let mut current = String::new();
        // Consume whitespace
        while let Some(&c) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            current.push(c);
            chars.next();
        }
        // Consume word characters
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

/// Apply a foreground color to all spans in lines that don't already have one.
pub fn apply_default_color(lines: &mut [Line<'static>], color: ratatui::style::Color) {
    for line in lines.iter_mut() {
        for span in &mut line.spans {
            if span.style.fg.is_none() {
                span.style = span.style.fg(color);
            }
        }
    }
}
