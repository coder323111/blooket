// main.rs — Blooket Claw CLI entry point

mod answer;
mod client;
mod crypto;
mod game;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use tracing_subscriber::EnvFilter;

use client::BlooketClient;
use game::GameRunner;
use types::{AutoMode, BlooketConfig};

#[derive(Parser)]
#[command(
    name = "blooket-engine",
    about = "Blooket Claw — automated Blooket game engine",
    version = "1.0.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Join and auto-complete a Blooket game
    Play {
        /// Game PIN
        #[arg(short, long)]
        pin: String,

        /// Player display name
        #[arg(short, long, default_value = "BlooketClaw")]
        name: String,

        /// Mode: auto | hint | speed
        #[arg(short, long, default_value = "auto")]
        mode: String,

        /// Base answer delay in ms (auto mode, adds jitter)
        #[arg(short, long, default_value_t = 1500)]
        delay: u64,

        /// Anthropic API key for LLM answers (overrides env)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: Option<String>,
    },

    /// Validate a game PIN without joining
    Check {
        /// Game PIN to validate
        pin: String,
    },

    /// Answer a single question via LLM (for testing)
    Ask {
        /// The question text
        question: String,

        /// Comma-separated answer choices
        #[arg(short, long)]
        choices: String,

        /// Anthropic API key
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("BLOOKET_LOG")
                .unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .without_time()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Play { pin, name, mode, delay, api_key } => {
            let auto_mode = match mode.as_str() {
                "hint"  => AutoMode::Hint,
                "speed" => AutoMode::Speed,
                _       => AutoMode::Auto,
            };

            let config = BlooketConfig {
                game_pin: pin,
                name,
                mode: auto_mode,
                delay_ms: delay,
                llm_endpoint: Some("https://api.anthropic.com/v1/messages".to_string()),
                llm_api_key: api_key.or_else(|| std::env::var("ANTHROPIC_API_KEY").ok()),
            };

            let client = BlooketClient::new(config)?;
            let mut runner = GameRunner::new(client);
            let result = runner.run().await?;

            // Output JSON result for Python layer to consume
            println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
        }

        Commands::Check { pin } => {
            let config = BlooketConfig {
                game_pin: pin.clone(),
                name: "checker".to_string(),
                mode: AutoMode::Auto,
                delay_ms: 0,
                llm_endpoint: None,
                llm_api_key: None,
            };
            let client = BlooketClient::new(config)?;
            match client.validate_pin().await {
                Ok(info) => {
                    println!("{}", format!("✅ PIN {} is valid!", pin).green().bold());
                    println!(
                        "   Mode: {}  |  Host: {}",
                        info["gameMode"].as_str().unwrap_or("?"),
                        info["host"].as_str().unwrap_or("?")
                    );
                }
                Err(e) => {
                    println!("{}", format!("❌ Invalid PIN: {}", e).red().bold());
                    std::process::exit(1);
                }
            }
        }

        Commands::Ask { question, choices, api_key } => {
            let key = api_key
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .unwrap_or_default();

            if key.is_empty() {
                eprintln!("{}", "❌ ANTHROPIC_API_KEY not set".red());
                std::process::exit(1);
            }

            let choices_vec: Vec<types::Answer> = choices
                .split(',')
                .enumerate()
                .map(|(i, c)| types::Answer {
                    index: i,
                    text: c.trim().to_string(),
                    is_correct: false,
                })
                .collect();

            let q = types::Question {
                id: "cli".to_string(),
                text: question.clone(),
                answers: choices_vec,
                correct_index: None,
                question_type: types::QuestionType::MultipleChoice,
                time_limit: None,
            };

            println!("\n{}", format!("❓ {}", question).bright_cyan());
            println!("{}", "🤖 Asking LLM...".dim());

            match answer::ask_llm(
                &q,
                "https://api.anthropic.com/v1/messages",
                &key,
            )
            .await
            {
                Ok((idx, confidence)) => {
                    println!(
                        "{}",
                        format!(
                            "\n✅ Answer: {}. {} ({:.0}% confident)",
                            idx,
                            q.answers[idx].text.bright_green(),
                            confidence * 100.0
                        )
                        .bold()
                    );
                }
                Err(e) => {
                    eprintln!("{}", format!("❌ LLM error: {}", e).red());
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
