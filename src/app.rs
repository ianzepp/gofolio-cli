use std::io;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tracing::info;

use crate::agent::client::{self, ModelEntry};
use crate::agent::types::{Content, Message, ToolCallRecord};
use crate::agent::{self, AgentEvent};
use crate::api::GhostfolioClient;
use crate::config::Config;
use crate::ui::login::{LoginField, LoginState};
use crate::ui::modal::ModalState;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    pub is_warning: bool,
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
    pub latency_ms: u64,
    pub feedback: Option<i8>, // 1 = thumbs up, -1 = thumbs down
    pub modal: Option<ModalState>,
    pub available_models: Vec<ModelEntry>,

    // Internal state
    config: Config,
    api_client: Option<GhostfolioClient>,
    history: Vec<Message>,
    agent_rx: Option<mpsc::UnboundedReceiver<AgentEvent>>,
    request_start: Option<Instant>,
}

impl AppState {
    fn new() -> Self {
        let config = Config::load();
        let model = config.model();

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
            latency_ms: 0,
            feedback: None,
            modal: None,
            available_models: Vec::new(),
            config,
            api_client: None,
            history: Vec::new(),
            agent_rx: None,
            request_start: None,
        }
    }

    fn push_system(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "system".to_string(),
            text: text.to_string(),
            is_warning: false,
        });
    }

    fn push_warning(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "system".to_string(),
            text: text.to_string(),
            is_warning: true,
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
        self.latency_ms = 0;
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
                    "Keys: ^N (new session), ^Y (thumbs up), ^R (report), ^P (model), ^T (traits), ^L (logout), ^Q (quit)\n\
                     Slash: /new, /up, /report, /model, /traits, /logout, /quit, /help",
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
        });

        // Add to conversation history
        self.history.push(Message {
            role: "user".to_string(),
            content: Content::Text(text),
        });

        self.tool_calls.clear();
        self.feedback = None;
        self.loading = true;
        self.request_start = Some(Instant::now());

        // Check prerequisites
        let Some(ref api_client) = self.api_client else {
            self.push_warning("Not connected to Ghostfolio.");
            self.loading = false;
            return;
        };
        let Some(api_key) = self.config.anthropic_api_key() else {
            self.push_warning("No ANTHROPIC_API_KEY configured.");
            self.loading = false;
            return;
        };

        // Spawn agent task
        let (tx, rx) = mpsc::unbounded_channel();
        self.agent_rx = Some(rx);

        agent::spawn_agent(
            api_client.clone(),
            api_key,
            self.model.clone(),
            self.history.clone(),
            tx,
        );
    }

    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::ToolCall(tc) => {
                self.tool_calls.push(tc);
            }
            AgentEvent::Response {
                text,
                input_tokens,
                output_tokens,
                steps,
            } => {
                self.loading = false;
                self.turn_count += 1;
                self.total_input_tokens += input_tokens;
                self.total_output_tokens += output_tokens;
                self.total_steps += steps;
                self.latency_ms = self
                    .request_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                self.messages.push(ChatMessage {
                    role: "agent".to_string(),
                    text: text.clone(),
                    is_warning: false,
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
        if let Some(api_key) = self.config.anthropic_api_key() {
            self.available_models = client::fetch_models(&api_key).await;
        }
    }

    async fn try_auto_auth(&mut self) {
        if self.config.access_token().is_none() {
            return;
        }

        match crate::api::auth::authenticate(&self.config).await {
            Ok((jwt, base_url)) => {
                self.api_client = Some(GhostfolioClient::new(base_url, jwt));
                self.screen = Screen::App;
                self.push_system("Connected. Type a message to begin.");
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

                self.api_client = Some(GhostfolioClient::new(url, jwt));
                self.screen = Screen::App;
                self.push_system("Connected. Type a message to begin.");
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
        let has_event = tokio::task::block_in_place(|| {
            event::poll(std::time::Duration::from_millis(50))
        })?;

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
                        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
                                        let selected = item
                                            .split(" (")
                                            .next()
                                            .unwrap_or(item)
                                            .to_string();
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
                            KeyCode::Char('q') => return Ok(()),
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
                        KeyCode::Char(c) => state.input.push(c),
                        _ => {}
                    }
                }
            }
        }
    }
}
