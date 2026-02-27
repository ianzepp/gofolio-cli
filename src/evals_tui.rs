use std::io;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use tokio::sync::mpsc;

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
        error: Option<String>,
    },
    AllDone,
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
}

struct TuiState {
    rows: Vec<RowState>,
    completed: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    total_cases: usize,
    suite_name: String,
    pool_size: usize,
    started_at: Instant,
    spinner_frame: usize,
    done: bool,
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl TuiState {
    fn new(suite_name: String, total_cases: usize, pool_size: usize) -> Self {
        Self {
            rows: Vec::new(),
            completed: 0,
            passed: 0,
            failed: 0,
            errors: 0,
            total_cases,
            suite_name,
            pool_size,
            started_at: Instant::now(),
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
                error,
            } => {
                if let Some(row) = self.rows.iter_mut().find(|r| r.case_id == case_id) {
                    row.status = if let Some(e) = error {
                        self.errors += 1;
                        RowStatus::Error(e)
                    } else if pass {
                        self.passed += 1;
                        RowStatus::Passed
                    } else {
                        self.failed += 1;
                        RowStatus::Failed
                    };
                }
                self.completed += 1;
            }
            TuiEvent::AllDone => {
                self.done = true;
            }
        }
    }

    fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER.len();
    }
}

/// Run the eval TUI. Blocks until all cases complete or the user presses q/Ctrl-C.
/// Renders to stderr so stdout stays clean.
pub async fn run_tui(
    mut rx: mpsc::UnboundedReceiver<TuiEvent>,
    suite_name: &str,
    total_cases: usize,
    pool_size: usize,
    model_label: &str,
) -> Result<(), String> {
    let mut stderr = io::stderr();
    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    stderr
        .execute(EnterAlternateScreen)
        .map_err(|e| format!("enter alt screen: {e}"))?;

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal init: {e}"))?;

    let mut state = TuiState::new(suite_name.to_string(), total_cases, pool_size);
    let model_label = model_label.to_string();

    let result = run_tui_loop(&mut terminal, &mut state, &mut rx, &model_label).await;

    // Restore terminal
    disable_raw_mode().ok();
    io::stderr()
        .execute(LeaveAlternateScreen)
        .ok();

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

        if state.done {
            break;
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
                if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    return Err("aborted by user".to_string());
                }
            }
        }

        state.tick();
    }

    Ok(())
}

fn render(frame: &mut ratatui::Frame, state: &TuiState, model_label: &str) {
    let area = frame.area();

    // Layout: header (2 lines), active rows (flexible), footer (2 lines)
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);

    render_header(frame, chunks[0], state, model_label);
    render_rows(frame, chunks[1], state);
    render_footer(frame, chunks[2], state);
}

fn render_header(frame: &mut ratatui::Frame, area: Rect, state: &TuiState, model_label: &str) {
    let elapsed = state.started_at.elapsed().as_secs();
    let header = Line::from(vec![
        Span::styled(
            format!(
                " Running suite '{}' ({} cases) — {} keys pooled — {}",
                state.suite_name, state.total_cases, state.pool_size, model_label,
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  [{elapsed}s]"),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let blank = Line::from("");
    let widget = Paragraph::new(vec![header, blank]);
    frame.render_widget(widget, area);
}

fn render_rows(frame: &mut ratatui::Frame, area: Rect, state: &TuiState) {
    let max_rows = area.height as usize;
    let spinner_char = SPINNER[state.spinner_frame];
    let width = area.width as usize;

    // Render all rows in order (completed + active), auto-scroll to bottom
    let all_lines: Vec<Line<'static>> = state
        .rows
        .iter()
        .map(|row| render_row(row, spinner_char, width))
        .collect();

    // Show the last N rows that fit in the viewport
    let skip = all_lines.len().saturating_sub(max_rows);
    let mut lines: Vec<Line<'static>> = all_lines.into_iter().skip(skip).collect();

    // Fill remaining lines with blanks
    while lines.len() < max_rows {
        lines.push(Line::from(""));
    }

    let widget = Paragraph::new(lines);
    frame.render_widget(widget, area);
}

fn render_row(row: &RowState, spinner_char: char, width: usize) -> Line<'static> {
    let case_id = format!(" {:<10}", row.case_id);
    let desc = if row.description.len() > 28 {
        format!("{:<28}", format!("{}...", &row.description[..25]))
    } else {
        format!("{:<28}", row.description)
    };

    // Build tool trail
    let mut tool_spans: Vec<Span<'static>> = Vec::new();
    for tool in &row.tools {
        let symbol = if tool.ok { " ✓" } else { " ✗" };
        let color = if tool.ok { Color::Green } else { Color::Red };
        tool_spans.push(Span::styled(
            format!(" {}{}", tool.name, symbol),
            Style::default().fg(color),
        ));
    }

    // Status on the right
    let status_span = match &row.status {
        RowStatus::Running => Span::styled(
            format!("  {spinner_char} "),
            Style::default().fg(Color::Yellow),
        ),
        RowStatus::Passed => Span::styled(
            " PASS ".to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        RowStatus::Failed => Span::styled(
            " FAIL ".to_string(),
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        RowStatus::Error(_) => Span::styled(
            " ERR  ".to_string(),
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
    };

    // Calculate used width for dots
    let case_width = case_id.len();
    let desc_width = desc.len();
    let tool_width: usize = tool_spans.iter().map(|s| s.content.len()).sum();
    let status_width = status_span.content.len();
    let used = case_width + desc_width + tool_width + status_width;
    let dots = if width > used + 2 {
        " ".repeat(1) + &".".repeat(width - used - 2) + " "
    } else {
        " ".to_string()
    };

    let mut spans = vec![
        Span::styled(case_id, Style::default().fg(Color::White)),
        Span::styled(desc, Style::default().fg(Color::DarkGray)),
    ];
    spans.extend(tool_spans);
    spans.push(Span::styled(dots, Style::default().fg(Color::DarkGray)));
    spans.push(status_span);

    Line::from(spans)
}

fn render_footer(frame: &mut ratatui::Frame, area: Rect, state: &TuiState) {
    let elapsed = state.started_at.elapsed().as_secs();
    let footer = Line::from(vec![
        Span::styled(" Completed: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", state.completed, state.total_cases),
            Style::default().fg(Color::White),
        ),
        Span::styled("  Passed: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", state.passed),
            Style::default().fg(Color::Green),
        ),
        Span::styled("  Failed: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", state.failed),
            Style::default().fg(if state.failed > 0 { Color::Red } else { Color::White }),
        ),
        Span::styled("  Errors: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", state.errors),
            Style::default().fg(if state.errors > 0 { Color::Red } else { Color::White }),
        ),
        Span::styled(
            format!("  Elapsed: {elapsed}s"),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let hint = Line::from(Span::styled(
        " Press q to abort",
        Style::default().fg(Color::DarkGray),
    ));
    let widget = Paragraph::new(vec![footer, hint]);
    frame.render_widget(widget, area);
}
