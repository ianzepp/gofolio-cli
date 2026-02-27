use std::io;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tracing::warn;

use crate::agent::client::{self, LlmClient, ModelEntry, Provider, ProviderConfig};
use crate::agent::types::{ConfidenceLabel, Content, Message, ToolCallRecord};
use crate::agent::{self, AgentEvent};
use crate::api::GhostfolioClient;
use crate::config::{Config, KeyFormatStatus, ProviderKeyStatus};
use crate::langsmith::LangSmithConfig;
use crate::market::{self, MarketQuote};
use crate::provider_cache;
use crate::ui::login::{LoginField, LoginState};
use crate::ui::modal::{ModalItem, ModalState};
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
    pub footer: Option<String>,
    pub footer_is_warning: bool,
}

#[derive(Debug)]
pub enum Screen {
    Login(LoginState),
    App,
}

#[derive(Debug, Clone)]
enum ModelModalValue {
    Model { provider: Provider, id: String },
}

pub struct AppState {
    pub screen: Screen,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub loading: bool,
    pub tool_calls: Vec<ToolCallRecord>,
    pub model: String,
    #[allow(dead_code)]
    pub traits: Vec<String>,
    pub turn_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_steps: usize,
    pub total_tool_calls: usize,
    pub latency_ms: u64,
    pub last_input_tokens: u64,
    pub verified: bool,
    pub confidence_label: ConfidenceLabel,
    pub confidence_score: f32,
    pub llm_keys_header: String,
    pub feedback: Option<i8>, // 1 = thumbs up, -1 = thumbs down
    pub scroll_offset: u16,   // 0 = at bottom, >0 = scrolled up by N rows
    pub modal: Option<ModalState>,
    pub market_quotes: Vec<MarketQuote>,
    pub portfolio: Option<PortfolioSummary>,

    // Internal state
    config: Config,
    llm_providers: Vec<ProviderConfig>,
    provider_key_statuses: Vec<ProviderKeyStatus>,
    llm_clients: Vec<(Provider, LlmClient)>,
    provider_models: Vec<(Provider, Vec<ModelEntry>)>,
    active_provider: Option<Provider>,
    model_modal_values: Vec<Option<ModelModalValue>>,
    api_client: Option<GhostfolioClient>,
    history: Vec<Message>,
    agent_rx: Option<mpsc::UnboundedReceiver<AgentEvent>>,
    agent_task: Option<tokio::task::JoinHandle<()>>,
    request_start: Option<Instant>,
    cancel_esc_at: Option<Instant>,
    warmup_rx: Option<tokio::sync::oneshot::Receiver<PortfolioSummary>>,
    market_rx: Option<mpsc::UnboundedReceiver<Vec<MarketQuote>>>,
    langsmith: Option<LangSmithConfig>,
}

impl AppState {
    fn header_keys_from_statuses(
        statuses: &[ProviderKeyStatus],
        langsmith: Option<&LangSmithConfig>,
    ) -> String {
        let mut parts = Vec::new();
        for s in statuses {
            let mark = if !s.configured {
                "✗"
            } else {
                match s.format {
                    Some(KeyFormatStatus::Expected) => "✓",
                    Some(KeyFormatStatus::LooksLike(_)) | Some(KeyFormatStatus::Unknown) => "!",
                    None => "✓",
                }
            };
            parts.push(format!("{}{}", s.provider.label(), mark));
        }
        let langchain_mark = if langsmith.is_some() { "✓" } else { "✗" };
        parts.push(format!("LangChain{langchain_mark}"));
        parts.join("  ")
    }

    fn new() -> Self {
        let config = Config::load();
        let langsmith = LangSmithConfig::from_config(&config);
        let provider_key_statuses = config.provider_key_statuses();
        let llm_providers = config.configured_llm_providers();
        let llm_clients = build_llm_clients(&llm_providers);
        let active_provider = config
            .preferred_llm_provider(&llm_providers)
            .filter(|p| llm_clients.iter().any(|(provider, _)| provider == p));
        let model = active_provider
            .map(|p| config.model_for_provider(p))
            .unwrap_or_else(|| config.model_for_provider(Provider::Anthropic));
        let llm_keys_header =
            Self::header_keys_from_statuses(&provider_key_statuses, langsmith.as_ref());

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
            verified: false,
            confidence_label: ConfidenceLabel::Low,
            confidence_score: 0.0,
            llm_keys_header,
            feedback: None,
            scroll_offset: 0,
            modal: None,
            market_quotes: Vec::new(),
            portfolio: None,
            config,
            llm_providers,
            provider_key_statuses,
            llm_clients,
            provider_models: Vec::new(),
            active_provider,
            model_modal_values: Vec::new(),
            api_client: None,
            history: Vec::new(),
            agent_rx: None,
            agent_task: None,
            request_start: None,
            cancel_esc_at: None,
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
            footer: None,
            footer_is_warning: false,
        });
    }

    fn push_warning(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "system".to_string(),
            text: text.to_string(),
            is_warning: true,
            chart: None,
            footer: None,
            footer_is_warning: false,
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
        self.verified = false;
        self.confidence_label = ConfidenceLabel::Low;
        self.confidence_score = 0.0;
        self.feedback = None;
        self.loading = false;
        if let Some(handle) = self.agent_task.take() {
            handle.abort();
        }
        self.agent_rx = None;
        self.cancel_esc_at = None;
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
            footer: None,
            footer_is_warning: false,
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
        self.cancel_esc_at = None;

        // Check prerequisites
        let Some(ref api_client) = self.api_client else {
            self.push_warning("Not connected to Ghostfolio.");
            self.loading = false;
            return;
        };
        let Some(provider) = self.active_provider else {
            self.push_warning(
                "No LLM API key configured. Set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or OPENAI_API_KEY.",
            );
            self.loading = false;
            return;
        };
        let Some(llm_client) = self.client_for_provider(provider).cloned() else {
            self.push_warning("Configured provider client is unavailable.");
            self.loading = false;
            return;
        };

        // Spawn agent task
        let (tx, rx) = mpsc::unbounded_channel();
        self.agent_rx = Some(rx);
        self.agent_task = Some(agent::spawn_agent(
            api_client.clone(),
            llm_client,
            self.model.clone(),
            self.history.clone(),
            self.langsmith.clone(),
            tx,
        ));
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
                        footer: None,
                        footer_is_warning: false,
                    });
                }
            }
            AgentEvent::Response {
                text,
                input_tokens,
                output_tokens,
                last_input_tokens,
                steps,
                verified,
                confidence_label,
                confidence_score,
            } => {
                self.loading = false;
                self.agent_task = None;
                self.agent_rx = None;
                self.cancel_esc_at = None;
                self.scroll_offset = 0;
                self.turn_count += 1;
                self.total_input_tokens += input_tokens;
                self.total_output_tokens += output_tokens;
                self.last_input_tokens = last_input_tokens;
                self.total_steps += steps;
                self.verified = verified;
                self.confidence_score = confidence_score;
                self.latency_ms = self
                    .request_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                let confidence_label_text = match &confidence_label {
                    ConfidenceLabel::High => "high",
                    ConfidenceLabel::Medium => "medium",
                    ConfidenceLabel::Low => "low",
                };

                self.messages.push(ChatMessage {
                    role: "agent".to_string(),
                    text: text.clone(),
                    is_warning: false,
                    chart: None,
                    footer: Some(format!(
                        "verify:{}  confidence:{:.0}% ({})",
                        if verified { "pass" } else { "warn" },
                        confidence_score * 100.0,
                        confidence_label_text
                    )),
                    footer_is_warning: !verified,
                });
                self.confidence_label = confidence_label;

                // Add assistant response to history
                self.history.push(Message {
                    role: "assistant".to_string(),
                    content: Content::Text(text),
                });

            }
            AgentEvent::Error(err) => {
                self.loading = false;
                self.agent_task = None;
                self.agent_rx = None;
                self.cancel_esc_at = None;
                self.latency_ms = self
                    .request_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                self.push_warning(&format!("Error: {err}"));
            }
        }
    }

    fn client_for_provider(&self, provider: Provider) -> Option<&LlmClient> {
        self.llm_clients
            .iter()
            .find_map(|(p, c)| (*p == provider).then_some(c))
    }

    fn models_for_provider(&self, provider: Provider) -> &[ModelEntry] {
        self.provider_models
            .iter()
            .find_map(|(p, models)| (*p == provider).then_some(models.as_slice()))
            .unwrap_or(&[])
    }

    fn set_models_for_provider(&mut self, provider: Provider, models: Vec<ModelEntry>) {
        if let Some((_, existing)) = self
            .provider_models
            .iter_mut()
            .find(|(p, _)| *p == provider)
        {
            *existing = models;
        } else {
            self.provider_models.push((provider, models));
        }
    }

    fn open_model_modal(&mut self) {
        let mut items: Vec<ModalItem> = Vec::new();
        let mut values = Vec::new();
        let mut missing_notes = Vec::new();
        for status in &self.provider_key_statuses {
            if !status.configured {
                missing_notes.push(format!(
                    "Missing {} ({})",
                    status.provider.label(),
                    Config::provider_env_var(status.provider)
                ));
                continue;
            }

            if self.client_for_provider(status.provider).is_none() {
                continue;
            }
            let heading = match &status.format {
                Some(KeyFormatStatus::Expected) => {
                    format!("{}", status.provider.label().to_uppercase())
                }
                Some(KeyFormatStatus::LooksLike(other)) => {
                    format!(
                        "{} (key looks like {})",
                        status.provider.label().to_uppercase(),
                        other.label()
                    )
                }
                Some(KeyFormatStatus::Unknown) => {
                    format!(
                        "{} (unrecognized key format)",
                        status.provider.label().to_uppercase()
                    )
                }
                None => status.provider.label().to_uppercase(),
            };
            items.push(ModalItem {
                text: heading,
                selectable: false,
            });
            values.push(None);

            let models = self.models_for_provider(status.provider);
            let model_rows: Vec<ModelEntry> = if models.is_empty() {
                vec![ModelEntry {
                    id: client::default_model_for_provider(status.provider).to_string(),
                    display_name: "default".to_string(),
                    input_cost_per_token: None,
                    output_cost_per_token: None,
                }]
            } else {
                models.to_vec()
            };
            for model in model_rows {
                items.push(ModalItem {
                    text: format!("• {}", model.id),
                    selectable: true,
                });
                values.push(Some(ModelModalValue::Model {
                    provider: status.provider,
                    id: model.id,
                }));
            }

            items.push(ModalItem {
                text: String::new(),
                selectable: false,
            });
            values.push(None);
        }

        if !missing_notes.is_empty() {
            if !items.is_empty()
                && !matches!(items.last(), Some(ModalItem { text, .. }) if text.is_empty())
            {
                items.push(ModalItem {
                    text: String::new(),
                    selectable: false,
                });
                values.push(None);
            }
            for note in missing_notes {
                items.push(ModalItem {
                    text: note,
                    selectable: false,
                });
                values.push(None);
            }
        }

        if items.is_empty() {
            items.push(ModalItem {
                text: "No providers configured".to_string(),
                selectable: false,
            });
            values.push(None);
        }

        self.model_modal_values = values;
        self.modal = Some(ModalState::new_items("Select Model".to_string(), items));
        if let Some(ref mut modal) = self.modal {
            modal.normalize_selection();
        }
    }

    async fn load_models_for_provider(&mut self, provider: Provider) {
        if let Some(c) = self.client_for_provider(provider) {
            let from_api = c.fetch_models().await;
            if from_api.is_empty() {
                self.set_models_for_provider(
                    provider,
                    provider_cache::load(provider).unwrap_or_default(),
                );
            } else {
                provider_cache::save(provider, &from_api);
                self.set_models_for_provider(provider, from_api);
            }
        }
    }

    async fn load_all_models(&mut self) {
        let providers: Vec<Provider> = self.llm_providers.iter().map(|c| c.provider).collect();
        for provider in providers {
            self.load_models_for_provider(provider).await;
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
                if let Some(provider) = self.active_provider {
                    self.model = self.config.model_for_provider(provider);
                    self.load_all_models().await;
                }
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
                if let Some(provider) = self.active_provider {
                    self.model = self.config.model_for_provider(provider);
                    self.load_all_models().await;
                }
            }
            Err(e) => {
                if let Screen::Login(ref mut ls) = self.screen {
                    ls.authenticating = false;
                    ls.error = Some(e.to_string());
                }
            }
        }
    }

    fn cancel_active_request(&mut self) {
        if !self.loading {
            return;
        }
        if let Some(handle) = self.agent_task.take() {
            handle.abort();
        }
        self.agent_rx = None;
        self.loading = false;
        self.cancel_esc_at = None;
        self.latency_ms = self
            .request_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        self.push_warning("Request canceled.");
    }
}

fn build_llm_clients(providers: &[ProviderConfig]) -> Vec<(Provider, LlmClient)> {
    let mut clients = Vec::new();
    for cfg in providers {
        match client::create_client(cfg) {
            Ok(client) => clients.push((cfg.provider, client)),
            Err(e) => {
                warn!(
                    provider = cfg.provider.id(),
                    error = %e,
                    "app: failed to initialize llm provider client"
                );
            }
        }
    }
    clients
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

        if let Some(ref mut rx) = state.warmup_rx {
            if let Ok(summary) = rx.try_recv() {
                state.portfolio = Some(summary);
                state.push_system("Portfolio data loaded. Type a message to begin.");
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
                                state.model_modal_values.clear();
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
                                    if let Some((orig_idx, _item)) = filtered.get(m.selected)
                                        && let Some(Some(value)) =
                                            state.model_modal_values.get(*orig_idx).cloned()
                                    {
                                        match value {
                                            ModelModalValue::Model { provider, id } => {
                                                state.active_provider = Some(provider);
                                                state.model = id.clone();
                                                state.config.llm_provider =
                                                    Some(provider.id().to_string());
                                                state.config.model = Some(id.clone());
                                                state.config.model_provider =
                                                    Some(provider.id().to_string());
                                                state.config.save();
                                                state.push_system(&format!(
                                                    "Provider set to {}. Model set to {}",
                                                    provider.label(),
                                                    id
                                                ));
                                            }
                                        }
                                    }
                                }
                                state.modal = None;
                                state.model_modal_values.clear();
                            }
                            KeyCode::Backspace => {
                                if let Some(ref mut m) = state.modal {
                                    m.filter.pop();
                                    m.selected = 0;
                                    m.normalize_selection();
                                }
                            }
                            KeyCode::Char(c) => {
                                if let Some(ref mut m) = state.modal {
                                    m.filter.push(c);
                                    m.selected = 0;
                                    m.normalize_selection();
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
                        KeyCode::Esc => {
                            if state.loading {
                                let now = Instant::now();
                                let should_cancel = state
                                    .cancel_esc_at
                                    .map(|t| now.duration_since(t).as_millis() <= 900)
                                    .unwrap_or(false);
                                if should_cancel {
                                    state.cancel_active_request();
                                } else {
                                    state.cancel_esc_at = Some(now);
                                    state.push_system("Press Esc again quickly to cancel request.");
                                }
                            }
                        }
                        KeyCode::Enter => state.submit_message(),
                        KeyCode::Backspace => {
                            state.input.pop();
                            state.cancel_esc_at = None;
                        }
                        KeyCode::PageUp => {
                            state.scroll_offset = state.scroll_offset.saturating_add(10);
                            state.cancel_esc_at = None;
                        }
                        KeyCode::PageDown => {
                            state.scroll_offset = state.scroll_offset.saturating_sub(10);
                            state.cancel_esc_at = None;
                        }
                        KeyCode::Home => {
                            // Scroll to top — use a large value; render will clamp
                            state.scroll_offset = u16::MAX;
                            state.cancel_esc_at = None;
                        }
                        KeyCode::End => {
                            state.scroll_offset = 0;
                            state.cancel_esc_at = None;
                        }
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            state.scroll_offset = state.scroll_offset.saturating_add(1);
                            state.cancel_esc_at = None;
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            state.scroll_offset = state.scroll_offset.saturating_sub(1);
                            state.cancel_esc_at = None;
                        }
                        KeyCode::Char(c) => {
                            state.input.push(c);
                            state.cancel_esc_at = None;
                        }
                        _ => {
                            state.cancel_esc_at = None;
                        }
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
