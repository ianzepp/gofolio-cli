mod agent;
mod api;
mod app;
mod config;
mod langsmith;
mod market;
mod markdown;
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
                            "model" => cfg.model = Some(value.to_string()),
                            "langchain_api_key" => {
                                cfg.langchain_api_key = Some(value.to_string());
                            }
                            "langchain_project" => {
                                cfg.langchain_project = Some(value.to_string());
                            }
                            _ => {
                                eprintln!("Unknown config key: {key}");
                                eprintln!("Valid keys: url, token, anthropic_api_key, model, langchain_api_key, langchain_project");
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
                    auth.and_then(|a| a.url.as_deref())
                        .unwrap_or("(not set)")
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
                    "model          = {}",
                    cfg.model.as_deref().unwrap_or("claude-sonnet-4-6")
                );
                println!(
                    "langchain_key  = {}",
                    if cfg.langchain_api_key().is_some() {
                        "***"
                    } else {
                        "(not set)"
                    }
                );
                println!(
                    "langchain_proj = {}",
                    cfg.langchain_project()
                );
                if let Some(ref traits) = cfg.traits
                    && !traits.is_empty()
                {
                    println!("traits         = {}", traits.join(", "));
                }
            }
        }
    }
}
