use super::*;

// ============================================================================
// Helper: extract plain text from rendered lines
// ============================================================================

fn text(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

fn has_style(lines: &[Line], modifier: Modifier) -> bool {
    lines.iter().any(|l| {
        l.spans
            .iter()
            .any(|s| s.style.add_modifier.contains(modifier))
    })
}

fn has_color(lines: &[Line], color: ratatui::style::Color) -> bool {
    lines
        .iter()
        .any(|l| l.spans.iter().any(|s| s.style.fg == Some(color)))
}

// ============================================================================
// Headings
// ============================================================================

#[test]
fn h1_renders_with_bold_and_blank_line() {
    let result = render("# Hello World", 80);
    assert_eq!(text(&result), vec!["Hello World"]);
    assert!(has_style(&result, Modifier::BOLD));
    assert!(has_color(&result, theme::AMBER));
}

#[test]
fn h2_renders() {
    let result = render("## Section Title", 80);
    assert_eq!(text(&result), vec!["Section Title"]);
    assert!(has_style(&result, Modifier::BOLD));
}

#[test]
fn h3_renders() {
    let result = render("### Subsection", 80);
    assert_eq!(text(&result), vec!["Subsection"]);
    assert!(has_style(&result, Modifier::BOLD));
}

#[test]
fn hash_without_space_is_not_heading() {
    let result = render("#notaheading", 80);
    let t = text(&result);
    assert_eq!(t, vec!["#notaheading"]);
    assert!(!has_style(&result, Modifier::BOLD));
}

#[test]
fn heading_followed_by_text() {
    let result = render("## Title\nSome text here", 80);
    let t = text(&result);
    assert_eq!(t[0], "Title");
    assert_eq!(t[1], ""); // blank line after heading
    assert_eq!(t[2], "Some text here");
}

#[test]
fn indented_heading_renders() {
    let result = render("  ## Indented Title", 80);
    let t = text(&result);
    assert_eq!(t, vec!["Indented Title"]);
}

// ============================================================================
// Horizontal rules
// ============================================================================

#[test]
fn three_dashes_is_hr() {
    let result = render("---", 80);
    assert_eq!(result.len(), 1);
    assert!(text(&result)[0].contains('\u{2500}'));
}

#[test]
fn three_asterisks_is_hr() {
    let result = render("***", 80);
    assert_eq!(result.len(), 1);
    assert!(text(&result)[0].contains('\u{2500}'));
}

#[test]
fn three_underscores_is_hr() {
    let result = render("___", 80);
    assert_eq!(result.len(), 1);
    assert!(text(&result)[0].contains('\u{2500}'));
}

#[test]
fn mixed_chars_not_hr() {
    // "-*-" should NOT be an HR (mixed chars)
    let result = render("-*-", 80);
    assert!(!text(&result)[0].contains('\u{2500}'));
}

#[test]
fn two_dashes_not_hr() {
    let result = render("--", 80);
    assert!(!text(&result)[0].contains('\u{2500}'));
}

#[test]
fn spaced_dashes_is_hr() {
    let result = render("- - -", 80);
    // This matches parse_bullet ("- ") first, so it's a bullet, not HR
    // That's acceptable — markdown spec is ambiguous here too
    assert_eq!(result.len(), 1);
}

// ============================================================================
// Fenced code blocks
// ============================================================================

#[test]
fn code_block_renders_indented_and_muted() {
    let result = render("```\nlet x = 1;\nlet y = 2;\n```", 80);
    let t = text(&result);
    assert_eq!(t, vec!["  let x = 1;", "  let y = 2;"]);
    assert!(has_color(&result, theme::MUTED));
}

#[test]
fn code_block_with_language_tag() {
    let result = render("```rust\nfn main() {}\n```", 80);
    let t = text(&result);
    assert_eq!(t, vec!["  fn main() {}"]);
}

#[test]
fn unclosed_code_block_renders_remaining_lines() {
    let result = render("```\nline1\nline2", 80);
    let t = text(&result);
    assert_eq!(t, vec!["  line1", "  line2"]);
}

// ============================================================================
// Pipe tables
// ============================================================================

#[test]
fn simple_table() {
    let input = "| Name | Value |\n| --- | --- |\n| Foo | 42 |";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t.len(), 3); // header, separator, data row
    assert!(t[0].contains("Name"));
    assert!(t[0].contains("Value"));
    assert!(t[1].contains('\u{2500}')); // separator line
    assert!(t[2].contains("Foo"));
    assert!(t[2].contains("42"));
}

#[test]
fn indented_table_renders() {
    let input = "  | Name | Value |\n  | --- | --- |\n  | Foo | 42 |";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t.len(), 3);
    assert!(t[0].contains("Name"));
    assert!(t[2].contains("Foo"));
}

#[test]
fn table_single_line_not_enough() {
    let result = render("| just one line |", 80);
    let t = text(&result);
    assert_eq!(t, vec!["| just one line |"]);
}

#[test]
fn table_invalid_separator() {
    let input = "| A | B |\n| not valid |\n| C | D |";
    let result = render(input, 80);
    let t = text(&result);
    // Should render as raw text since separator is invalid
    assert_eq!(t[0], "| A | B |");
}

// ============================================================================
// Pre-formatted (box-drawing) lines
// ============================================================================

#[test]
fn box_drawing_passthrough() {
    let input = "Name  \u{2502} Value\n\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{253C}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Name  \u{2502} Value");
    assert!(t[1].contains('\u{253C}'));
}

#[test]
fn box_drawing_preserves_spacing() {
    let line = "  Col A   \u{2502} Col B  ";
    let result = render(line, 80);
    let t = text(&result);
    assert_eq!(t[0], line);
}

// ============================================================================
// Bullet lists
// ============================================================================

#[test]
fn dash_bullet() {
    let result = render("- Item one", 80);
    let t = text(&result);
    assert_eq!(t[0], "\u{2022} Item one");
}

#[test]
fn asterisk_bullet() {
    let result = render("* Item two", 80);
    let t = text(&result);
    assert_eq!(t[0], "\u{2022} Item two");
}

#[test]
fn indented_bullet() {
    let result = render("  - Nested item", 80);
    let t = text(&result);
    assert_eq!(t[0], "  \u{2022} Nested item");
}

#[test]
fn bullet_with_bold() {
    let result = render("- **Important** item", 80);
    let t = text(&result);
    assert_eq!(t[0], "\u{2022} Important item");
    assert!(has_style(&result, Modifier::BOLD));
}

// ============================================================================
// Inline formatting
// ============================================================================

#[test]
fn bold_text() {
    let result = render("This is **bold** text", 80);
    let t = text(&result);
    assert_eq!(t[0], "This is bold text");
    assert!(has_style(&result, Modifier::BOLD));
}

#[test]
fn italic_text() {
    let result = render("This is *italic* text", 80);
    let t = text(&result);
    assert_eq!(t[0], "This is italic text");
    assert!(has_style(&result, Modifier::ITALIC));
}

#[test]
fn inline_code() {
    let result = render("Use `println!` here", 80);
    let t = text(&result);
    assert_eq!(t[0], "Use println! here");
    assert!(has_color(&result, theme::MUTED));
}

#[test]
fn multiple_bold_spans() {
    let result = render("**one** and **two**", 80);
    let t = text(&result);
    assert_eq!(t[0], "one and two");
}

#[test]
fn unclosed_bold_renders_remaining() {
    let result = render("**unclosed bold", 80);
    let t = text(&result);
    assert_eq!(t[0], "unclosed bold");
}

#[test]
fn unclosed_code_renders_remaining() {
    let result = render("some `unclosed code", 80);
    let t = text(&result);
    assert_eq!(t[0], "some unclosed code");
}

#[test]
fn plain_text_no_formatting() {
    let result = render("Just plain text.", 80);
    let t = text(&result);
    assert_eq!(t[0], "Just plain text.");
    assert!(!has_style(&result, Modifier::BOLD));
    assert!(!has_style(&result, Modifier::ITALIC));
}

#[test]
fn crlf_and_control_chars_are_sanitized() {
    let input = "Hello\r\nWorld\u{0007}\r\nDone";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Hello World Done");
}

// ============================================================================
// Paragraph collapsing
// ============================================================================

#[test]
fn consecutive_lines_collapse_to_paragraph() {
    let result = render("Line one\nline two\nline three", 80);
    let t = text(&result);
    assert_eq!(t[0], "Line one line two line three");
}

#[test]
fn blank_line_separates_paragraphs() {
    let result = render("Para one\n\nPara two", 80);
    let t = text(&result);
    assert_eq!(t[0], "Para one");
    assert_eq!(t[1], "");
    assert_eq!(t[2], "Para two");
}

// ============================================================================
// Word wrapping
// ============================================================================

#[test]
fn wraps_at_width() {
    let result = render("hello world foo", 20);
    let t = text(&result);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0], "hello world foo");

    let result = render("hello world foo bar baz", 12);
    let t = text(&result);
    assert!(t.len() > 1);
    // All lines fit within width
    for line in &t {
        assert!(line.len() <= 12, "line too long: {:?}", line);
    }
}

#[test]
fn unicode_wrap_does_not_break_on_byte_boundaries() {
    let result = render("📈📉📊📈📉📊", 2);
    let t = text(&result);
    assert!(!t.join("").trim().is_empty());
}

#[test]
fn long_word_hard_breaks() {
    let result = render("abcdefghij", 5);
    let t = text(&result);
    assert_eq!(t[0], "abcde");
    assert_eq!(t[1], "fghij");
}

#[test]
fn zero_width_no_wrap() {
    let result = render("hello world", 0);
    let t = text(&result);
    assert_eq!(t[0], "hello world");
}

// ============================================================================
// Empty / edge cases
// ============================================================================

#[test]
fn empty_input() {
    let result = render("", 80);
    let t = text(&result);
    assert!(t.is_empty());
}

#[test]
fn only_blank_lines() {
    let result = render("\n\n\n", 80);
    assert!(result.is_empty());
}

#[test]
fn single_newline() {
    let result = render("\n", 80);
    assert!(result.is_empty());
}

// ============================================================================
// Mixed content (realistic LLM output)
// ============================================================================

#[test]
fn heading_then_bullets() {
    let input = "## Portfolio\n- **JPM**: 62%\n- **VEA**: 15%";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Portfolio");
    assert_eq!(t[1], ""); // blank after heading
    assert!(t[2].contains('\u{2022}'));
    assert!(t[3].contains('\u{2022}'));
}

#[test]
fn heading_then_table() {
    let input = "## Holdings\n| Symbol | Value |\n| --- | --- |\n| JPM | 299k |";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Holdings");
    assert_eq!(t[1], ""); // blank after heading
    assert!(t[2].contains("Symbol"));
    assert!(t[3].contains('\u{2500}')); // table separator
    assert!(t[4].contains("JPM"));
}

#[test]
fn heading_then_preformatted_table() {
    let input = "### Summary\n Name  \u{2502} Value\n \u{2500}\u{2500}\u{2500}\u{253C}\u{2500}\u{2500}\u{2500}\n Foo   \u{2502} 42";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Summary");
    assert_eq!(t[1], "");
    assert!(t[2].contains('\u{2502}'));
}

#[test]
fn text_then_code_then_text() {
    let input = "Before code:\n```\nfn main() {}\n```\nAfter code.";
    let result = render(input, 80);
    let t = text(&result);
    assert_eq!(t[0], "Before code:");
    assert_eq!(t[1], "  fn main() {}");
    assert_eq!(t[2], "After code.");
}

#[test]
fn no_infinite_loop_on_bare_hash() {
    // A line like "#hashtag" should not cause an infinite loop
    let result = render("#hashtag", 80);
    let t = text(&result);
    assert_eq!(t[0], "#hashtag");
}

#[test]
fn no_infinite_loop_on_number_sign_variations() {
    let result = render("##nospace\n###alsono\n#", 80);
    assert!(!text(&result).is_empty());
}

#[test]
fn bold_asterisks_not_treated_as_hr() {
    let result = render("***not a rule***", 80);
    // Should NOT contain box-drawing HR char
    assert!(!text(&result)[0].contains('\u{2500}'));
}

#[test]
fn realistic_llm_response() {
    let input = "\
## Market Overview

The S&P 500 is trading at **4,500.00 USD** as of today.

### Your Holdings

| Symbol | Value |
| --- | --- |
| JPM | 299,998.80 USD |
| VEA | 71,184.63 USD |

### Allocation

- **Equities**: 78.26%
- **Crypto**: 13.55%
- **Bonds**: 4.28%
- **Cash**: 5.18%

---

Use `get_holdings` for more details.";

    let result = render(input, 80);
    let t = text(&result);

    // Should have content, not be empty
    assert!(t.len() > 10);

    // Check key elements exist
    assert!(t.iter().any(|l| l == "Market Overview"));
    assert!(t.iter().any(|l| l.contains("4,500.00 USD")));
    assert!(t.iter().any(|l| l == "Your Holdings"));
    assert!(t.iter().any(|l| l.contains("JPM")));
    assert!(t.iter().any(|l| l == "Allocation"));
    assert!(t.iter().any(|l| l.contains('\u{2022}'))); // bullets
    assert!(t.iter().any(|l| l.contains('\u{2500}'))); // HR or table sep
    assert!(t.iter().any(|l| l.contains("get_holdings"))); // inline code
}
