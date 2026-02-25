use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split sidebar: markets (top) + portfolio (middle) + tools (bottom, fill)
    let market_height = if state.market_quotes.is_empty() {
        2 // header + "(loading...)"
    } else {
        (state.market_quotes.len() as u16) + 2 // header + quotes + blank line
    };

    let portfolio_height = match &state.portfolio {
        None => 2,    // header + "(loading...)"
        Some(p) => {
            let mut h: u16 = 1; // header
            if p.total_value.is_some() { h += 1; }
            if p.total_investment.is_some() { h += 1; }
            if p.net_performance.is_some() { h += 1; }
            h += 1; // holdings/accounts line
            if !p.top_holdings.is_empty() {
                h += 1; // blank line before top holdings
                h += p.top_holdings.len() as u16;
            }
            h + 1 // trailing space
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(market_height),
            Constraint::Length(1), // blank row
            Constraint::Length(portfolio_height),
            Constraint::Length(1), // blank row
            Constraint::Min(3),
        ])
        .split(area);

    render_market_panel(frame, chunks[0], state);
    render_portfolio_panel(frame, chunks[2], state);
    render_tools_panel(frame, chunks[4], state);
}

fn render_market_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            "MARKETS",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(1, 1, 1, 0));

    let mut lines = Vec::new();

    if state.market_quotes.is_empty() {
        lines.push(Line::from(Span::styled(
            "(loading...)",
            Style::default().fg(theme::MUTED),
        )));
    } else {
        for q in &state.market_quotes {
            let arrow = if q.change_pct >= 0.0 { "\u{25B2}" } else { "\u{25BC}" };
            let color = if q.change_pct >= 0.0 { theme::GREEN } else { theme::RED };

            let price_str = if q.price >= 1000.0 {
                format!("{:.0}", q.price)
            } else if q.price >= 100.0 {
                format!("{:.1}", q.price)
            } else {
                format!("{:.2}", q.price)
            };

            let name = format!("{:<7}", q.name);

            lines.push(Line::from(vec![
                Span::styled(name, Style::default().fg(theme::WHITE)),
                Span::styled(
                    format!("{:>8}", price_str),
                    Style::default().fg(theme::WHITE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(
                    format!("{} {:.1}%", arrow, q.change_pct.abs()),
                    Style::default().fg(color),
                ),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_portfolio_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            "PORTFOLIO",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(1, 1, 1, 0));

    let label_style = Style::default().fg(theme::MUTED);
    let value_style = Style::default().fg(theme::WHITE).add_modifier(Modifier::BOLD);

    let mut lines = Vec::new();

    match &state.portfolio {
        None => {
            lines.push(Line::from(Span::styled(
                "(loading...)",
                Style::default().fg(theme::MUTED),
            )));
        }
        Some(p) => {
            let cur = if p.currency.is_empty() { "USD" } else { &p.currency };

            if let Some(val) = p.total_value {
                lines.push(Line::from(vec![
                    Span::styled("Value  ", label_style),
                    Span::styled(format_money(val, cur), value_style),
                ]));
            }

            if let Some(inv) = p.total_investment {
                lines.push(Line::from(vec![
                    Span::styled("Invest ", label_style),
                    Span::styled(format_money(inv, cur), value_style),
                ]));
            }

            if let Some(perf) = p.net_performance {
                let pct_str = p.net_performance_pct
                    .map(|pct| format!(" ({:.1}%)", pct * 100.0))
                    .unwrap_or_default();
                let color = if perf >= 0.0 { theme::GREEN } else { theme::RED };
                let arrow = if perf >= 0.0 { "\u{25B2}" } else { "\u{25BC}" };
                lines.push(Line::from(vec![
                    Span::styled("P&L    ", label_style),
                    Span::styled(
                        format!("{} {}{}", arrow, format_money(perf.abs(), cur), pct_str),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} holdings, {} accounts", p.num_holdings, p.num_accounts),
                    label_style,
                ),
            ]));

            if !p.top_holdings.is_empty() {
                lines.push(Line::from(""));
                for h in &p.top_holdings {
                    // Truncate long names to fit sidebar
                    let name = if h.name.len() > 18 {
                        format!("{}...", &h.name[..15])
                    } else {
                        h.name.clone()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("{:<19}", name), Style::default().fg(theme::WHITE)),
                        Span::styled(
                            format!("{:>5.1}%", h.allocation_pct),
                            label_style,
                        ),
                    ]));
                }
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_tools_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            "TOOLS",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(1, 1, 1, 0));

    let mut lines = Vec::new();

    if state.tool_calls.is_empty() && !state.loading {
        lines.push(Line::from(Span::styled(
            "(none)",
            Style::default().fg(theme::MUTED),
        )));
    }

    for tc in &state.tool_calls {
        let check = if tc.success { "\u{2713}" } else { "\u{2717}" };
        let color = if tc.success { theme::GREEN } else { theme::RED };
        lines.push(Line::from(Span::styled(
            tc.name.clone(),
            Style::default().fg(theme::WHITE),
        )));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}ms ", tc.duration_ms),
                Style::default().fg(theme::MUTED),
            ),
            Span::styled(check.to_string(), Style::default().fg(color)),
        ]));
    }

    if state.loading {
        lines.push(Line::from(vec![
            Span::styled("\u{25CF} ", Style::default().fg(theme::AMBER)),
            Span::styled("running..", Style::default().fg(theme::MUTED)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn format_money(amount: f64, currency: &str) -> String {
    let abs = amount.abs();
    if abs >= 1_000_000.0 {
        format!("{:.1}M {}", abs / 1_000_000.0, currency)
    } else if abs >= 1_000.0 {
        // Format with comma separators
        let whole = abs as u64;
        let frac = ((abs - whole as f64) * 100.0).round() as u64;
        let formatted = format_with_commas(whole);
        format!("{}.{:02} {}", formatted, frac, currency)
    } else {
        format!("{:.2} {}", abs, currency)
    }
}

fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}
