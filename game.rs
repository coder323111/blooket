// game.rs — Game loop orchestrator for Blooket

use anyhow::Result;
use colored::*;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::answer::{ask_llm, format_hint, select_answer};
use crate::client::BlooketClient;
use crate::crypto::human_delay_ms;
use crate::types::*;

pub struct GameRunner {
    client: BlooketClient,
    state: GameState,
    start_time: Instant,
}

impl GameRunner {
    pub fn new(client: BlooketClient) -> Self {
        let name = client.config.name.clone();
        let pin = client.config.game_pin.clone();
        Self {
            client,
            state: GameState::new(&pin, &name),
            start_time: Instant::now(),
        }
    }

    /// Main entry point — run the full game session
    pub async fn run(&mut self) -> Result<SessionResult> {
        self.print_banner();

        // Validate PIN
        println!("{}", "🔍 Validating game PIN...".bright_blue());
        let game_info = self.client.validate_pin().await?;
        let set_id = game_info["setId"].as_str().unwrap_or("").to_string();
        self.state.game_mode = game_info["gameMode"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        println!(
            "{}",
            format!(
                "✅ Game found! Mode: {} | Set: {}",
                self.state.game_mode.bright_yellow(),
                set_id
            )
            .green()
        );

        // Pre-load question set if available
        let mut questions: Vec<Question> = vec![];
        if !set_id.is_empty() {
            println!("{}", "📚 Loading question set...".bright_blue());
            questions = self.client.fetch_questions(&set_id).await.unwrap_or_default();
            if !questions.is_empty() {
                println!(
                    "{}",
                    format!("📖 Loaded {} questions with answers", questions.len()).green()
                );
            }
        }

        // Join the game
        println!(
            "{}",
            format!("🎮 Joining as '{}'...", self.client.config.name).bright_blue()
        );
        let game_token = self.client.join_game().await?;
        println!("{}", "✅ Joined! Waiting for game to start...".green());

        // Game loop
        self.game_loop(&game_token, &questions).await?;

        let duration = self.start_time.elapsed().as_secs();
        let result = SessionResult {
            pin: self.state.pin.clone(),
            name: self.state.player_name.clone(),
            final_score: self.state.score,
            questions_total: self.state.questions_answered,
            correct: self.state.correct,
            accuracy: self.state.accuracy(),
            mode: self.client.config.mode.to_string(),
            duration_secs: duration,
        };

        self.print_summary(&result);
        Ok(result)
    }

    async fn game_loop(&mut self, token: &str, prefetched: &[Question]) -> Result<()> {
        let mode = self.client.config.mode.clone();
        let delay = self.client.config.delay_ms;

        loop {
            // Poll for next question
            let state = match self.client.get_game_state(token).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("State poll error: {}", e);
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Check if game ended
            if state["gameOver"].as_bool().unwrap_or(false) {
                println!("\n{}", "🏁 Game over!".bright_green().bold());
                break;
            }

            // Extract current question
            let question = match extract_question(&state, prefetched) {
                Some(q) => q,
                None => {
                    sleep(Duration::from_millis(300)).await;
                    continue;
                }
            };

            // Skip if already answered this question
            if Some(&question.id) == self.state.current_question.as_ref().map(|q| &q.id) {
                sleep(Duration::from_millis(200)).await;
                continue;
            }

            self.state.current_question = Some(question.clone());
            self.state.questions_answered += 1;

            let llm_endpoint = self.client.config.llm_endpoint.as_deref();
            let llm_key = self.client.config.llm_api_key.as_deref();

            let (answer_idx, confidence) =
                select_answer(&question, llm_endpoint, llm_key).await?;

            match mode {
                AutoMode::Hint => {
                    // Just show the hint, don't auto-answer
                    println!("{}", format_hint(&question, answer_idx, confidence));
                    // Wait for human to answer manually
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                AutoMode::Auto | AutoMode::Speed => {
                    let answer_delay = if mode == AutoMode::Speed {
                        50
                    } else {
                        human_delay_ms(delay)
                    };

                    println!(
                        "{}",
                        format!(
                            "\n❓ {} [{}]",
                            &question.text.chars().take(80).collect::<String>(),
                            format!("{:.0}%", confidence * 100.0).bright_cyan()
                        )
                    );
                    println!(
                        "{}",
                        format!(
                            "   → Answering: '{}' in {}ms",
                            question.answers[answer_idx].text.bright_green(),
                            answer_delay
                        )
                    );

                    sleep(Duration::from_millis(answer_delay)).await;

                    match self
                        .client
                        .submit_answer(token, &question.id, answer_idx, answer_delay)
                        .await
                    {
                        Ok(resp) => {
                            let correct = resp["correct"].as_bool().unwrap_or(false);
                            let points = resp["points"].as_i64().unwrap_or(0);
                            self.state.score += points;
                            if correct {
                                self.state.correct += 1;
                                self.state.streak += 1;
                                println!(
                                    "{}",
                                    format!(
                                        "   ✅ Correct! +{} pts | Score: {} | Streak: {}",
                                        points, self.state.score, self.state.streak
                                    )
                                    .bright_green()
                                );
                            } else {
                                self.state.incorrect += 1;
                                self.state.streak = 0;
                                println!(
                                    "{}",
                                    format!("   ❌ Wrong! Score: {}", self.state.score)
                                        .bright_red()
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Submit answer error: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn print_banner(&self) {
        println!();
        println!("{}", "╔══════════════════════════════════════════╗".bright_cyan());
        println!("{}", "║         🎮  B L O O K E T  C L A W       ║".bright_cyan().bold());
        println!("{}", "╚══════════════════════════════════════════╝".bright_cyan());
        println!(
            "  PIN:  {}   Name: {}   Mode: {}",
            self.client.config.game_pin.bright_yellow(),
            self.client.config.name.bright_green(),
            self.client.config.mode.to_string().bright_magenta()
        );
        println!();
    }

    fn print_summary(&self, result: &SessionResult) {
        println!();
        println!("{}", "═══════════════════════════════════════════".bright_cyan());
        println!("{}", "  📊  SESSION SUMMARY".bold());
        println!("{}", "═══════════════════════════════════════════".bright_cyan());
        println!("  Score:     {}", result.final_score.to_string().bright_yellow().bold());
        println!(
            "  Accuracy:  {}/{} ({:.0}%)",
            result.correct,
            result.questions_total,
            result.accuracy
        );
        println!("  Duration:  {}s", result.duration_secs);
        println!("  Mode:      {}", result.mode.bright_magenta());
        println!("{}", "═══════════════════════════════════════════".bright_cyan());
        println!();
    }
}

fn extract_question(state: &serde_json::Value, prefetched: &[Question]) -> Option<Question> {
    let q_data = state.get("currentQuestion")?;
    let q_id = q_data.get("id")?.as_str()?.to_string();

    // Try to find in prefetched (has known correct answer)
    if let Some(q) = prefetched.iter().find(|q| q.id == q_id) {
        return Some(q.clone());
    }

    // Parse from live state
    let text = q_data.get("question")?.as_str()?.to_string();
    let answers_raw = q_data.get("answers")?.as_array()?;

    let answers: Vec<Answer> = answers_raw
        .iter()
        .enumerate()
        .map(|(i, a)| Answer {
            index: i,
            text: a.get("answer")
                .or_else(|| a.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            is_correct: false,
        })
        .collect();

    Some(Question {
        id: q_id,
        text,
        correct_index: None, // Unknown — will use LLM
        question_type: QuestionType::MultipleChoice,
        answers,
        time_limit: q_data.get("timeLimit").and_then(|v| v.as_u64()).map(|v| v as u32),
    })
}
