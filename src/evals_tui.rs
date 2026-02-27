use std::io;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::theme;

/// Events sent from eval worker tasks to the TUI.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    CaseStarted {
        case_id: String,
        description: String,
    },
    ToolDone {
        case_id: String,
        tool_name: String,
        ok: bool,
    },
    CaseFinished {
        case_id: String,
        pass: bool,
        detail: CaseDetail,
    },
    AllDone,
}

/// Detail info attached to a finished case for the detail modal.
#[derive(Debug, Clone, Default)]
pub struct CaseDetail {
    pub error: Option<String>,
    pub response: Option<String>,
    pub tier_a: Option<String>,
    pub tier_b: Option<String>,
    pub tier_c: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum RowStatus {
    Running,
    Passed,
    Failed,
    Error(String),
}

#[derive(Debug, Clone)]
struct ToolEntry {
    name: String,
    ok: bool,
}

#[derive(Debug, Clone)]
struct RowState {
    case_id: String,
    description: String,
    tools: Vec<ToolEntry>,
    status: RowStatus,
    detail: CaseDetail,
}

struct TuiState {
    rows: Vec<RowState>,
    selected: usize,
    show_detail: bool,
    detail_scroll: u16,
    completed: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    total_cases: usize,
    suite_name: String,
    key_count: usize,
    max_concurrent: usize,
    started_at: Instant,
    elapsed_secs: u64,
    spinner_frame: usize,
    done: bool,
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl TuiState {
    fn new(suite_name: String, total_cases: usize, key_count: usize, max_concurrent: usize) -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            show_detail: false,
            detail_scroll: 0,
            completed: 0,
            passed: 0,
            failed: 0,
            errors: 0,
            total_cases,
            suite_name,
            key_count,
            max_concurrent,
            started_at: Instant::now(),
            elapsed_secs: 0,
            spinner_frame: 0,
            done: false,
        }
    }

    fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::CaseStarted {
                case_id,
                description,
            } => {
                self.rows.push(RowState {
                    case_id,
                    description,
                    tools: Vec::new(),
                    status: RowStatus::Running,
                    detail: CaseDetail::default(),
                });
            }
            TuiEvent::ToolDone {
                case_id,
                tool_name,
                ok,
            } => {
                if let Some(row) = self.rows.iter_mut().find(|r| r.case_id == case_id) {
                    row.tools.push(ToolEntry {
                        name: tool_name,
                        ok,
                    });
                }
            }
            TuiEvent::CaseFinished {
                case_id,
                pass,
                detail,
            } => {
                if let Some(row) = self.rows.iter_mut().find(|r| r.case_id == case_id) {
                    row.status = if detail.error.is_some() {
                        self.errors += 1;
                        RowStatus::Error(detail.error.clone().unwrap_or_default())
                    } else if pass {
                        self.passed += 1;
                        RowStatus::Passed
                    } else {
                        self.failed += 1;
                        RowStatus::Failed
                    };
                    row.detail = detail;
                }
                self.completed += 1;
            }
            TuiEvent::AllDone => {
                self.elapsed_secs = self.started_at.elapsed().as_secs();
                self.done = true;
            }
        }
    }

    fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER.len();
        if !self.done {
            self.elapsed_secs = self.started_at.elapsed().as_secs();
        }
    }
}

/// Run the eval TUI. Blocks until all cases complete or the user presses q/Ctrl-C.
/// Renders to stderr so stdout stays clean.
pub async fn run_tui(
    mut rx: mpsc::UnboundedReceiver<TuiEvent>,
    suite_name: &str,
    total_cases: usize,
    key_count: usize,
    max_concurrent: usize,
    model_label: &str,
) -> Result<(), String> {
    let mut stderr = io::stderr();
    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    stderr
        .execute(EnterAlternateScreen)
        .map_err(|e| format!("enter alt screen: {e}"))?;

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal init: {e}"))?;

    let mut state = TuiState::new(suite_name.to_string(), total_cases, key_count, max_concurrent);
    let model_label = model_label.to_string();

    let result = run_tui_loop(&mut terminal, &mut state, &mut rx, &model_label).await;

    // Restore terminal
    disable_raw_mode().ok();
    io::stderr().execute(LeaveAlternateScreen).ok();

    result
}

async fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    state: &mut TuiState,
    rx: &mut mpsc::UnboundedReceiver<TuiEvent>,
    model_label: &str,
) -> Result<(), String> {
    loop {
        // Drain all pending events
        while let Ok(ev) = rx.try_recv() {
            state.handle_event(ev);
        }

        // Render
        terminal
            .draw(|frame| render(frame, state, model_label))
            .map_err(|e| format!("render: {e}"))?;

        // Poll for keyboard input with 100ms timeout (tick rate)
        if crossterm::event::poll(std::time::Duration::from_millis(100))
            .map_err(|e| format!("poll: {e}"))?
        {
            if let Ok(Event::Key(key)) = event::read() {
                if state.show_detail {
                    // Detail modal is open
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            state.show_detail = false;
                            state.detail_scroll = 0;
                        }
                        KeyCode::Up => {
                            state.detail_scroll = state.detail_scroll.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            state.detail_scroll += 1;
                        }
                        _ => {}
                    }
                } else if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    if state.done {
                        return Ok(());
                    }
                    return Err("aborted by user".to_string());
                } else {
                    match key.code {
                        KeyCode::Up => {
                            state.selected = state.selected.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            if !state.rows.is_empty() {
                                state.selected =
                                    (state.selected + 1).min(state.rows.len() - 1);
                            }
                        }
                        KeyCode::Enter => {
                            if !state.rows.is_empty()
                                && !matches!(state.rows[state.selected].status, RowStatus::Running)
                            {
                                state.show_detail = true;
                                state.detail_scroll = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        state.tick();
    }
}

fn render(frame: &mut ratatui::Frame, state: &TuiState, model_label: &str) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);

    render_header(frame, chunks[0], state, model_label);
    render_rows(frame, chunks[1], state);
    render_footer(frame, chunks[2], state);

    if state.show_detail && !state.rows.is_empty() {
        render_detail_modal(frame, area, &state.rows[state.selected], state.detail_scroll);
    }
}

fn render_header(frame: &mut ratatui::Frame, area: Rect, state: &TuiState, model_label: &str) {
    let elapsed = state.elapsed_secs;
    let w = area.width as usize;
    let header_style = Style::default()
        .fg(theme::BG)
        .bg(theme::AMBER)
        .add_modifier(Modifier::BOLD);

    let left = format!(
        " Running suite '{}' ({} cases) — {} keys, {} concurrent — {}",
        state.suite_name, state.total_cases, state.key_count, state.max_concurrent, model_label,
    );
    let right = format!("[{elapsed}s] ");
    let pad = w.saturating_sub(left.len() + right.len());
    let header = Line::from(Span::styled(
        format!("{left}{}{right}", " ".repeat(pad)),
        header_style,
    ));

    let blank = Line::from("");
    let widget = Paragraph::new(vec![header, blank]);
    frame.render_widget(widget, area);
}

fn render_rows(frame: &mut ratatui::Frame, area: Rect, state: &TuiState) {
    let max_rows = area.height as usize;
    let spinner_char = SPINNER[state.spinner_frame];
    let width = area.width as usize;

    // Ensure selected row is visible by adjusting the scroll window
    let total = state.rows.len();
    let skip = if total <= max_rows {
        0
    } else if state.selected < max_rows / 2 {
        0
    } else if state.selected > total - max_rows / 2 {
        total - max_rows
    } else {
        state.selected - max_rows / 2
    };

    let lines: Vec<Line<'static>> = state
        .rows
        .iter()
        .enumerate()
        .skip(skip)
        .take(max_rows)
        .map(|(i, row)| render_row(row, spinner_char, width, i == state.selected))
        .collect();

    let widget = Paragraph::new(lines);
    frame.render_widget(widget, area);
}

fn render_row(row: &RowState, spinner_char: char, width: usize, selected: bool) -> Line<'static> {
    let bg = if selected { theme::BORDER } else { theme::BG };

    // Status on the far left
    let status_span = match &row.status {
        RowStatus::Running => Span::styled(
            format!(" {spinner_char}    "),
            Style::default().fg(theme::WARNING).bg(bg),
        ),
        RowStatus::Passed => Span::styled(
            " PASS ".to_string(),
            Style::default()
                .fg(theme::GREEN)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        RowStatus::Failed => Span::styled(
            " FAIL ".to_string(),
            Style::default()
                .fg(theme::RED)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        RowStatus::Error(_) => Span::styled(
            " ERR  ".to_string(),
            Style::default()
                .fg(theme::RED)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let case_id = format!("{:<10}", row.case_id);
    let desc = if row.description.len() > 28 {
        format!("{:<28}", format!("{}...", &row.description[..25]))
    } else {
        format!("{:<28}", row.description)
    };

    // Build tool trail
    let mut tool_spans: Vec<Span<'static>> = Vec::new();
    let mut tool_display_width: usize = 0;
    for tool in &row.tools {
        let symbol = if tool.ok { " ✓" } else { " ✗" };
        let color = if tool.ok { theme::GREEN } else { theme::RED };
        let text = format!(" {}{}", tool.name, symbol);
        // Display width: space + name + space + 1-char symbol
        tool_display_width += 1 + tool.name.len() + 2;
        tool_spans.push(Span::styled(text, Style::default().fg(color).bg(bg)));
    }

    // Calculate used display width for dots
    let status_width = 6; // " PASS " / " FAIL " / " ERR  " / " ⠋    "
    let case_width = case_id.len();
    let desc_width = desc.len();
    let used = status_width + case_width + desc_width + tool_display_width;
    let dots = if width > used + 2 {
        " ".repeat(1) + &".".repeat(width - used - 2) + " "
    } else {
        " ".to_string()
    };

    let case_style = if selected {
        Style::default().fg(theme::WHITE).bg(bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::WHITE).bg(bg)
    };
    let mut spans = vec![
        status_span,
        Span::styled(case_id, case_style),
        Span::styled(desc, Style::default().fg(theme::MUTED).bg(bg)),
    ];
    spans.extend(tool_spans);
    spans.push(Span::styled(dots, Style::default().fg(theme::MUTED).bg(bg)));

    Line::from(spans)
}

fn render_footer(frame: &mut ratatui::Frame, area: Rect, state: &TuiState) {
    let elapsed = state.elapsed_secs;
    let footer = Line::from(vec![
        Span::styled(" Completed: ", Style::default().fg(theme::MUTED)),
        Span::styled(
            format!("{}/{}", state.completed, state.total_cases),
            Style::default().fg(theme::WHITE),
        ),
        Span::styled("  Passed: ", Style::default().fg(theme::MUTED)),
        Span::styled(
            format!("{}", state.passed),
            Style::default().fg(theme::GREEN),
        ),
        Span::styled("  Failed: ", Style::default().fg(theme::MUTED)),
        Span::styled(
            format!("{}", state.failed),
            Style::default().fg(if state.failed > 0 { theme::RED } else { theme::WHITE }),
        ),
        Span::styled("  Errors: ", Style::default().fg(theme::MUTED)),
        Span::styled(
            format!("{}", state.errors),
            Style::default().fg(if state.errors > 0 { theme::RED } else { theme::WHITE }),
        ),
        Span::styled(
            format!("  Elapsed: {elapsed}s"),
            Style::default().fg(theme::MUTED),
        ),
    ]);
    let hint_text = if state.done {
        " ↑↓: select  Enter: details  q: exit"
    } else {
        " ↑↓: select  Enter: details  q: abort"
    };
    let hint = Line::from(Span::styled(
        hint_text,
        Style::default().fg(theme::MUTED),
    ));
    let widget = Paragraph::new(vec![footer, hint]);
    frame.render_widget(widget, area);
}

fn render_detail_modal(frame: &mut ratatui::Frame, area: Rect, row: &RowState, scroll: u16) {
    // Center a modal covering ~80% of the screen
    let margin_x = area.width / 10;
    let margin_y = area.height / 10;
    let modal_area = Rect {
        x: area.x + margin_x,
        y: area.y + margin_y,
        width: area.width.saturating_sub(margin_x * 2),
        height: area.height.saturating_sub(margin_y * 2),
    };

    frame.render_widget(Clear, modal_area);

    let status_label = match &row.status {
        RowStatus::Passed => "PASS",
        RowStatus::Failed => "FAIL",
        RowStatus::Error(_) => "ERROR",
        RowStatus::Running => "RUNNING",
    };

    let block = Block::default()
        .title(Span::styled(
            format!(" {} — {} — {} ", row.case_id, row.description, status_label),
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::AMBER));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Tools used
    if !row.tools.is_empty() {
        lines.push(Line::from(Span::styled(
            "Tools:",
            Style::default()
                .fg(theme::WHITE)
                .add_modifier(Modifier::BOLD),
        )));
        for tool in &row.tools {
            let (symbol, color) = if tool.ok {
                ("✓", theme::GREEN)
            } else {
                ("✗", theme::RED)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {symbol} "), Style::default().fg(color)),
                Span::styled(tool.name.clone(), Style::default().fg(theme::WHITE)),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Tier grading details
    let detail = &row.detail;
    if let Some(ref a) = detail.tier_a {
        lines.push(Line::from(Span::styled(
            "Tier A (Tools):",
            Style::default()
                .fg(theme::WHITE)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {a}"),
            Style::default().fg(theme::MUTED),
        )));
        lines.push(Line::from(""));
    }
    if let Some(ref b) = detail.tier_b {
        lines.push(Line::from(Span::styled(
            "Tier B (Response):",
            Style::default()
                .fg(theme::WHITE)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {b}"),
            Style::default().fg(theme::MUTED),
        )));
        lines.push(Line::from(""));
    }
    if let Some(ref c) = detail.tier_c {
        lines.push(Line::from(Span::styled(
            "Tier C (Verification):",
            Style::default()
                .fg(theme::WHITE)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {c}"),
            Style::default().fg(theme::MUTED),
        )));
        lines.push(Line::from(""));
    }

    // Error message
    if let Some(ref err) = detail.error {
        lines.push(Line::from(Span::styled(
            "Error:",
            Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
        )));
        for line in err.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(theme::RED),
            )));
        }
        lines.push(Line::from(""));
    }

    // Agent response
    if let Some(ref resp) = detail.response {
        lines.push(Line::from(Span::styled(
            "Agent Response:",
            Style::default()
                .fg(theme::WHITE)
                .add_modifier(Modifier::BOLD),
        )));
        for line in resp.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(theme::MUTED),
            )));
        }
    }

    // Scroll hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ↑↓: scroll  Esc/Enter/q: close",
        Style::default().fg(theme::MUTED),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, inner);
}
