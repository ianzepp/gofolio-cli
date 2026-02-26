mod agent;
mod api;
mod app;
mod config;
mod evals;
mod langsmith;
mod markdown;
mod market;
mod provider_cache;
mod text;
mod theme;
mod tools;
mod ui;
mod warmup;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ghostfolio")]
#[command(about = "Bloomberg-terminal TUI for Ghostfolio with AI agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the interactive TUI (default)
    Chat,
    /// Run eval suites against the in-process CLI agent
    Test {
        /// Suite id from gauntlet/evals/suites.yaml
        #[arg(long, default_value = "quick")]
        suite: String,
        /// Comma-separated case ids to run (overrides suite case selection)
        #[arg(long = "case", value_delimiter = ',')]
        case_ids: Option<Vec<String>>,
        /// Model id override (defaults to configured provider model)
        #[arg(long, conflicts_with = "models")]
        model: Option<String>,
        /// Comma-separated model ids to run as a matrix (overrides --model)
        #[arg(long, value_delimiter = ',')]
        models: Option<Vec<String>>,
        /// LLM provider override (anthropic|openrouter|openai)
        #[arg(long)]
        provider: Option<String>,
        /// Evals root path (auto-detected if omitted)
        #[arg(long)]
        evals_root: Option<String>,
        /// Fixture directory for mock mode (defaults to evals/fixtures/moderate-portfolio)
        #[arg(long)]
        fixture_dir: Option<String>,
        /// Use live Ghostfolio API instead of fixture-backed mock data
        #[arg(long, default_value_t = false)]
        live: bool,
        /// Run test cases in parallel (in-process async tasks)
        #[arg(long, default_value_t = false)]
        parallel: bool,
        /// Maximum number of cases to run concurrently when --parallel is enabled
        #[arg(long)]
        max_parallel: Option<usize>,
        /// List available suites and exit
        #[arg(long, default_value_t = false)]
        list_suites: bool,
    },
    /// Show or edit configuration
    Config {
        /// Set a config key (e.g., ghostfolio_url=http://localhost:3333)
        #[arg(value_name = "KEY=VALUE")]
        set: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ghostfolio_cli=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Chat) {
        Command::Chat => {
            if let Err(e) = app::run().await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Command::Test {
            suite,
            case_ids,
            model,
            models,
            provider,
            evals_root,
            fixture_dir,
            live,
            parallel,
            max_parallel,
            list_suites,
        } => {
            let args = evals::TestArgs {
                suite,
                case_ids,
                model,
                models,
                provider,
                evals_root: evals_root.map(std::path::PathBuf::from),
                fixture_dir: fixture_dir.map(std::path::PathBuf::from),
                live,
                parallel,
                max_parallel,
                list_suites,
            };
            if let Err(e) = evals::run(args).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Command::Config { set } => {
            if let Some(kv) = set {
                match kv.split_once('=') {
                    Some((key, value)) => {
                        let mut cfg = config::Config::load();
                        match key {
                            "url" | "ghostfolio_url" => {
                                cfg.set_auth(Some(value.to_string()), None);
                            }
                            "token" | "access_token" => {
                                cfg.set_auth(None, Some(value.to_string()));
                            }
                            "anthropic_api_key" => {
                                cfg.anthropic_api_key = Some(value.to_string());
                            }
                            "openrouter_api_key" => {
                                cfg.openrouter_api_key = Some(value.to_string());
                            }
                            "openai_api_key" => {
                                cfg.openai_api_key = Some(value.to_string());
                            }
                            "llm_provider" => {
                                cfg.llm_provider = Some(value.to_string());
                            }
                            "llm_adapter" => {
                                cfg.llm_adapter = Some(value.to_string());
                            }
                            "model" => cfg.model = Some(value.to_string()),
                            "langchain_api_key" => {
                                cfg.langchain_api_key = Some(value.to_string());
                            }
                            "langchain_project" => {
                                cfg.langchain_project = Some(value.to_string());
                            }
                            _ => {
                                eprintln!("Unknown config key: {key}");
                                eprintln!(
                                    "Valid keys: url, token, anthropic_api_key, openrouter_api_key, openai_api_key, llm_provider, llm_adapter, model, langchain_api_key, langchain_project"
                                );
                                std::process::exit(1);
                            }
                        }
                        cfg.save();
                        println!("Set {key}");
                    }
                    None => {
                        eprintln!("Expected KEY=VALUE format");
                        std::process::exit(1);
                    }
                }
            } else {
                let cfg = config::Config::load();
                let path = config::Config::path();
                println!("Config file: {}", path.display());
                println!();
                let auth = cfg.auth.as_ref();
                println!(
                    "auth.url       = {}",
                    auth.and_then(|a| a.url.as_deref()).unwrap_or("(not set)")
                );
                println!(
                    "auth.token     = {}",
                    if auth.and_then(|a| a.token.as_ref()).is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!(
                    "anthropic_key  = {}",
                    if cfg.anthropic_api_key.is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!(
                    "openrouter_key = {}",
                    if cfg.openrouter_api_key.is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!(
                    "openai_key     = {}",
                    if cfg.openai_api_key.is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!(
                    "llm_provider   = {}",
                    cfg.llm_provider.as_deref().unwrap_or("(auto)")
                );
                println!(
                    "llm_adapter    = {}",
                    cfg.llm_adapter.as_deref().unwrap_or("(provider default)")
                );
                println!(
                    "model          = {}",
                    cfg.model.as_deref().unwrap_or("(provider default)")
                );
                println!(
                    "langchain_key  = {}",
                    if cfg.langchain_api_key().is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!("langchain_proj = {}", cfg.langchain_project());
                if let Some(ref traits) = cfg.traits
                    && !traits.is_empty()
                {
                    println!("traits         = {}", traits.join(", "));
                }
            }
        }
    }
}
