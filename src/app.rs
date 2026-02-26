use std::io;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tracing::info;

use crate::agent::client::{self, LlmClient, ModelEntry};
use crate::agent::types::{Content, Message, ToolCallRecord};
use crate::agent::{self, AgentEvent};
use crate::api::GhostfolioClient;
use crate::config::Config;
use crate::langsmith::LangSmithConfig;
use crate::market::{self, MarketQuote};
use crate::ui::login::{LoginField, LoginState};
use crate::ui::modal::ModalState;
use crate::warmup::{self, PortfolioSummary};

#[derive(Debug, Clone)]
pub enum ChartData {
    Sparkline {
        title: String,
        data: Vec<u64>,
    },
    Bar {
        title: String,
        labels: Vec<String>,
        values: Vec<u64>,
    },
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    pub is_warning: bool,
    pub chart: Option<ChartData>,
}

#[derive(Debug)]
pub enum Screen {
    Login(LoginState),
    App,
}

pub struct AppState {
    pub screen: Screen,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub loading: bool,
    pub tool_calls: Vec<ToolCallRecord>,
    pub model: String,
    pub traits: Vec<String>,
    pub turn_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_steps: usize,
    pub total_tool_calls: usize,
    pub latency_ms: u64,
    pub last_input_tokens: u64,
    pub feedback: Option<i8>, // 1 = thumbs up, -1 = thumbs down
    pub scroll_offset: u16,   // 0 = at bottom, >0 = scrolled up by N rows
    pub modal: Option<ModalState>,
    pub available_models: Vec<ModelEntry>,
    pub market_quotes: Vec<MarketQuote>,
    pub portfolio: Option<PortfolioSummary>,

    // Internal state
    config: Config,
    llm_client: Option<LlmClient>,
    api_client: Option<GhostfolioClient>,
    history: Vec<Message>,
    agent_rx: Option<mpsc::UnboundedReceiver<AgentEvent>>,
    request_start: Option<Instant>,
    warmup_rx: Option<tokio::sync::oneshot::Receiver<warmup::WarmupData>>,
    market_rx: Option<mpsc::UnboundedReceiver<Vec<MarketQuote>>>,
    langsmith: Option<LangSmithConfig>,
}

impl AppState {
    fn new() -> Self {
        let config = Config::load();
        let model = config.model();
        let langsmith = LangSmithConfig::from_config(&config);

        let llm_client = config
            .detect_llm_provider()
            .and_then(|(provider, key)| client::create_client(&provider, key).ok());

        // Check for pre-configured auth
        let has_token = config.access_token().is_some();
        let screen = if has_token {
            Screen::Login(LoginState {
                authenticating: true,
                ..LoginState::default()
            })
        } else {
            Screen::Login(LoginState::default())
        };

        Self {
            screen,
            messages: Vec::new(),
            input: String::new(),
            loading: false,
            tool_calls: Vec::new(),
            model,
            traits: Vec::new(),
            turn_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_steps: 0,
            total_tool_calls: 0,
            latency_ms: 0,
            last_input_tokens: 0,
            feedback: None,
            scroll_offset: 0,
            modal: None,
            available_models: Vec::new(),
            market_quotes: Vec::new(),
            portfolio: None,
            config,
            llm_client,
            api_client: None,
            history: Vec::new(),
            agent_rx: None,
            request_start: None,
            warmup_rx: None,
            market_rx: None,
            langsmith,
        }
    }

    fn push_system(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "system".to_string(),
            text: text.to_string(),
            is_warning: false,
            chart: None,
        });
    }

    fn push_warning(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "system".to_string(),
            text: text.to_string(),
            is_warning: true,
            chart: None,
        });
    }

    fn clear_session(&mut self) {
        self.messages.clear();
        self.history.clear();
        self.tool_calls.clear();
        self.turn_count = 0;
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
        self.total_steps = 0;
        self.total_tool_calls = 0;
        self.latency_ms = 0;
        self.last_input_tokens = 0;
        self.feedback = None;
        self.loading = false;
        self.push_system("Session cleared. Type a message to begin.");
    }

    fn submit_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.loading {
            return;
        }
        self.input.clear();

        // Handle slash commands
        match text.as_str() {
            "/new" | "/clear" => {
                self.clear_session();
                return;
            }
            "/quit" | "/exit" | "/q" => {
                // Signal quit — handled by caller checking messages
                self.push_system("Use Ctrl+Q to quit.");
                return;
            }
            "/help" | "/?" => {
                self.push_system(
                    "Keys: ^N (new session), ^Y (thumbs up), ^R (report), ^P (model), ^L (logout), ^Q (quit)\n\
                     Scroll: PgUp/PgDn, Shift+Up/Down, Home/End\n\
                     Slash: /new, /up, /report, /model, /logout, /quit, /help",
                );
                return;
            }
            "/up" => {
                self.feedback = Some(1);
                self.push_system("Feedback recorded: thumbs up");
                return;
            }
            "/report" => {
                self.push_system("Use Ctrl+R to report a problem.");
                return;
            }
            "/model" => {
                self.open_model_modal();
                return;
            }
            "/traits" => {
                self.push_system("Traits system available via Ctrl+T.");
                return;
            }
            "/logout" => {
                self.api_client = None;
                self.clear_session();
                self.messages.clear();
                self.screen = Screen::Login(LoginState::default());
                return;
            }
            _ => {}
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            text: text.clone(),
            is_warning: false,
            chart: None,
        });

        // Add to conversation history
        self.history.push(Message {
            role: "user".to_string(),
            content: Content::Text(text),
        });

        self.tool_calls.clear();
        self.feedback = None;
        self.scroll_offset = 0;
        self.loading = true;
        self.request_start = Some(Instant::now());

        // Check prerequisites
        let Some(ref api_client) = self.api_client else {
            self.push_warning("Not connected to Ghostfolio.");
            self.loading = false;
            return;
        };
        let Some(ref llm_client) = self.llm_client else {
            self.push_warning(
                "No LLM API key configured. Set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or OPENAI_API_KEY.",
            );
            self.loading = false;
            return;
        };

        // Spawn agent task
        let (tx, rx) = mpsc::unbounded_channel();
        self.agent_rx = Some(rx);

        agent::spawn_agent(
            api_client.clone(),
            llm_client.clone(),
            self.model.clone(),
            self.history.clone(),
            self.langsmith.clone(),
            tx,
        );
    }

    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::ToolCall(tc) => {
                self.tool_calls.push(tc);
                self.total_tool_calls += 1;
            }
            AgentEvent::ChartData(data) => {
                let chart = parse_chart_data(&data);
                if let Some(chart) = chart {
                    self.messages.push(ChatMessage {
                        role: "system".to_string(),
                        text: String::new(),
                        is_warning: false,
                        chart: Some(chart),
                    });
                }
            }
            AgentEvent::Response {
                text,
                input_tokens,
                output_tokens,
                last_input_tokens,
                steps,
            } => {
                self.loading = false;
                self.scroll_offset = 0;
                self.turn_count += 1;
                self.total_input_tokens += input_tokens;
                self.total_output_tokens += output_tokens;
                self.last_input_tokens = last_input_tokens;
                self.total_steps += steps;
                self.latency_ms = self
                    .request_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                self.messages.push(ChatMessage {
                    role: "agent".to_string(),
                    text: text.clone(),
                    is_warning: false,
                    chart: None,
                });

                // Add assistant response to history
                self.history.push(Message {
                    role: "assistant".to_string(),
                    content: Content::Text(text),
                });
            }
            AgentEvent::Error(err) => {
                self.loading = false;
                self.latency_ms = self
                    .request_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                self.push_warning(&format!("Error: {err}"));
            }
        }
    }

    fn open_model_modal(&mut self) {
        let items: Vec<String> = if self.available_models.is_empty() {
            // Fallback if models haven't loaded yet
            vec![
                "claude-opus-4-6".to_string(),
                "claude-sonnet-4-6".to_string(),
                "claude-haiku-4-5-20251001".to_string(),
            ]
        } else {
            self.available_models
                .iter()
                .map(|m| format!("{} ({})", m.id, m.display_name))
                .collect()
        };
        self.modal = Some(ModalState::new("Select Model".to_string(), items));
    }

    async fn load_models(&mut self) {
        if let Some(ref c) = self.llm_client {
            self.available_models = c.fetch_models().await;
        }
    }

    async fn try_auto_auth(&mut self) {
        if self.config.access_token().is_none() {
            return;
        }

        match crate::api::auth::authenticate(&self.config).await {
            Ok((jwt, base_url)) => {
                let client = GhostfolioClient::new(base_url, jwt);
                self.warmup_rx = Some(warmup::spawn_warmup(client.clone()));
                self.api_client = Some(client);
                self.screen = Screen::App;
                self.push_system("Connected. Loading portfolio data...");
                self.load_models().await;
            }
            Err(_) => {
                self.screen = Screen::Login(LoginState::default());
            }
        }
    }

    async fn try_login(&mut self, url: String, token: String) {
        // Strip trailing slashes from URL
        let url = url.trim_end_matches('/').to_string();
        let http = reqwest::Client::new();
        match crate::api::auth::exchange_token(&http, &url, &token).await {
            Ok(jwt) => {
                // Save to config (deep-merge auth like original)
                self.config.set_auth(Some(url.clone()), Some(token));
                self.config.save();

                let client = GhostfolioClient::new(url, jwt);
                self.warmup_rx = Some(warmup::spawn_warmup(client.clone()));
                self.api_client = Some(client);
                self.screen = Screen::App;
                self.push_system("Connected. Loading portfolio data...");
                self.load_models().await;
            }
            Err(e) => {
                if let Screen::Login(ref mut ls) = self.screen {
                    ls.authenticating = false;
                    ls.error = Some(e.to_string());
                }
            }
        }
    }
}

pub async fn run() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal).await;
    ratatui::restore();
    result
}

async fn run_loop(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut state = AppState::new();

    // Start market data feed (no auth needed)
    let (market_tx, market_rx) = mpsc::unbounded_channel();
    state.market_rx = Some(market_rx);
    market::spawn_market_feed(market_tx);

    // Try auto-auth if we have a token
    if matches!(
        state.screen,
        Screen::Login(LoginState {
            authenticating: true,
            ..
        })
    ) {
        state.try_auto_auth().await;
    }

    loop {
        terminal.draw(|frame| crate::ui::render(frame, &state))?;

        // Poll for events with a short timeout so we can check agent channel
        let has_event =
            tokio::task::block_in_place(|| event::poll(std::time::Duration::from_millis(50)))?;

        // Check warmup channel
        if let Some(ref mut rx) = state.warmup_rx {
            if let Ok(data) = rx.try_recv() {
                state.portfolio = Some(data.portfolio);
                if !data.context.is_empty() {
                    // Inject as a prefilled user→assistant exchange so the LLM has context
                    state.history.push(Message {
                        role: "user".to_string(),
                        content: Content::Text(data.context),
                    });
                    state.history.push(Message {
                        role: "assistant".to_string(),
                        content: Content::Text(
                            "Understood. I have your portfolio data loaded and ready. How can I help?".to_string(),
                        ),
                    });
                    state.push_system("Portfolio data loaded. Type a message to begin.");
                } else {
                    state.push_system("Ready. Type a message to begin.");
                }
                state.warmup_rx = None;
            }
        }

        // Check market feed channel
        if let Some(ref mut rx) = state.market_rx {
            if let Ok(quotes) = rx.try_recv() {
                state.market_quotes = quotes;
            }
        }

        // Check agent channel — collect events first to avoid double borrow
        let mut agent_events = Vec::new();
        if let Some(ref mut rx) = state.agent_rx {
            while let Ok(event) = rx.try_recv() {
                agent_events.push(event);
            }
        }
        for event in agent_events {
            state.handle_agent_event(event);
        }

        if !has_event {
            continue;
        }

        let event = tokio::task::block_in_place(event::read)?;

        match &mut state.screen {
            Screen::Login(login) => {
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('c')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        KeyCode::Esc => return Ok(()),
                        KeyCode::Tab | KeyCode::BackTab | KeyCode::Down | KeyCode::Up => {
                            login.focus = match login.focus {
                                LoginField::Url => LoginField::Token,
                                LoginField::Token => LoginField::Url,
                            };
                        }
                        KeyCode::Enter => {
                            if !login.authenticating {
                                let url = login.url.clone();
                                let token = login.token.clone();
                                login.authenticating = true;
                                login.error = None;
                                state.try_login(url, token).await;
                            }
                        }
                        KeyCode::Backspace => match login.focus {
                            LoginField::Url => {
                                login.url.pop();
                            }
                            LoginField::Token => {
                                login.token.pop();
                            }
                        },
                        KeyCode::Char(c) => match login.focus {
                            LoginField::Url => login.url.push(c),
                            LoginField::Token => login.token.push(c),
                        },
                        _ => {}
                    }
                }
            }
            Screen::App => {
                if let Event::Key(key) = event {
                    // Modal handling
                    if state.modal.is_some() {
                        match key.code {
                            KeyCode::Esc => {
                                state.modal = None;
                            }
                            KeyCode::Up => {
                                if let Some(ref mut m) = state.modal {
                                    m.move_up();
                                }
                            }
                            KeyCode::Down => {
                                if let Some(ref mut m) = state.modal {
                                    m.move_down();
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(ref m) = state.modal {
                                    let filtered = m.filtered_items();
                                    if let Some((_idx, item)) = filtered.get(m.selected) {
                                        // Extract model ID (before " (" if display name appended)
                                        let selected =
                                            item.split(" (").next().unwrap_or(item).to_string();
                                        info!(selected = %selected, "modal: selected");
                                        state.model = selected.clone();
                                        state.config.model = Some(selected.clone());
                                        state.config.save();
                                        state.push_system(&format!("Model set to {selected}"));
                                    }
                                }
                                state.modal = None;
                            }
                            KeyCode::Backspace => {
                                if let Some(ref mut m) = state.modal {
                                    m.filter.pop();
                                    m.selected = 0;
                                }
                            }
                            KeyCode::Char(c) => {
                                if let Some(ref mut m) = state.modal {
                                    m.filter.push(c);
                                    m.selected = 0;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Ctrl shortcuts
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('c') => return Ok(()),
                            KeyCode::Char('n') => state.clear_session(),
                            KeyCode::Char('y') => {
                                state.feedback = Some(1);
                                state.push_system("Feedback recorded: thumbs up");
                            }
                            KeyCode::Char('r') => {
                                state.push_system("Report noted.");
                                state.feedback = Some(-1);
                            }
                            KeyCode::Char('l') => {
                                state.api_client = None;
                                state.clear_session();
                                state.messages.clear();
                                state.screen = Screen::Login(LoginState::default());
                            }
                            KeyCode::Char('p') => {
                                state.open_model_modal();
                            }
                            KeyCode::Char('t') => {
                                state.push_system("Traits: not yet available in standalone mode.");
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Normal input
                    match key.code {
                        KeyCode::Enter => state.submit_message(),
                        KeyCode::Backspace => {
                            state.input.pop();
                        }
                        KeyCode::PageUp => {
                            state.scroll_offset = state.scroll_offset.saturating_add(10);
                        }
                        KeyCode::PageDown => {
                            state.scroll_offset = state.scroll_offset.saturating_sub(10);
                        }
                        KeyCode::Home => {
                            // Scroll to top — use a large value; render will clamp
                            state.scroll_offset = u16::MAX;
                        }
                        KeyCode::End => {
                            state.scroll_offset = 0;
                        }
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            state.scroll_offset = state.scroll_offset.saturating_add(1);
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            state.scroll_offset = state.scroll_offset.saturating_sub(1);
                        }
                        KeyCode::Char(c) => state.input.push(c),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn parse_chart_data(data: &serde_json::Value) -> Option<ChartData> {
    match data["chart_type"].as_str()? {
        "sparkline" => {
            let title = data["title"].as_str().unwrap_or("Chart").to_string();
            let values: Vec<u64> = data["data"]
                .as_array()?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f.max(0.0) as u64))
                .collect();
            Some(ChartData::Sparkline {
                title,
                data: values,
            })
        }
        "bar" => {
            let title = data["title"].as_str().unwrap_or("Chart").to_string();
            let labels: Vec<String> = data["labels"]
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let values: Vec<u64> = data["values"]
                .as_array()?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f.max(0.0) as u64))
                .collect();
            Some(ChartData::Bar {
                title,
                labels,
                values,
            })
        }
        _ => None,
    }
}
