// client.rs — Blooket HTTP and WebSocket client

use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::types::*;
use crate::crypto::*;

pub const BLOOKET_API: &str = "https://fb-rest.blooket.com";
pub const BLOOKET_DB:  &str = "https://fb-db.blooket.com";

pub struct BlooketClient {
    http: reqwest::Client,
    pub config: BlooketConfig,
    pub player_id: String,
    pub firebase_token: Option<String>,
}

impl BlooketClient {
    pub fn new(config: BlooketConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/122.0.0.0 Safari/537.36"
        ));
        headers.insert(
            "x-blooket-client",
            HeaderValue::from_static("web-2.0"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let player_id = random_player_id();

        Ok(Self {
            http,
            config,
            player_id,
            firebase_token: None,
        })
    }

    /// Check that a game PIN is valid and the game is joinable
    pub async fn validate_pin(&self) -> Result<Value> {
        let url = format!(
            "{}/games/pin?pin={}",
            BLOOKET_API, self.config.game_pin
        );
        debug!("Validating PIN at {}", url);

        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "Invalid PIN or game not active (HTTP {})",
                resp.status()
            ));
        }

        let data: Value = resp.json().await?;
        info!("Game found: {:?}", data.get("gameMode"));
        Ok(data)
    }

    /// Fetch the question set for a game
    pub async fn fetch_questions(&self, set_id: &str) -> Result<Vec<Question>> {
        let url = format!("{}/sets/public?id={}", BLOOKET_API, set_id);
        debug!("Fetching question set: {}", url);

        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            warn!("Could not fetch question set: {}", resp.status());
            return Ok(vec![]);
        }

        let data: Value = resp.json().await?;
        let questions = parse_question_set(&data);
        info!("Loaded {} questions", questions.len());
        Ok(questions)
    }

    /// Join a game as a named player
    pub async fn join_game(&self) -> Result<String> {
        let url = format!("{}/games/join", BLOOKET_API);
        let payload = build_join_payload(
            &self.config.game_pin,
            &self.config.name,
            &self.player_id,
        );

        debug!("Joining game {} as '{}'", self.config.game_pin, self.config.name);

        let resp = self.http
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow!(
                "Failed to join game: {} — {:?}",
                status,
                body.get("msg")
            ));
        }

        let token = body["token"]
            .as_str()
            .unwrap_or("")
            .to_string();

        info!("Joined game, token: {}...", &token.chars().take(12).collect::<String>());
        Ok(token)
    }

    /// Submit an answer for a question
    pub async fn submit_answer(
        &self,
        game_token: &str,
        question_id: &str,
        answer_index: usize,
        time_taken_ms: u64,
    ) -> Result<Value> {
        let url = format!("{}/games/answer", BLOOKET_API);
        let payload = serde_json::json!({
            "token": game_token,
            "questionId": question_id,
            "answer": answer_index,
            "timeTaken": time_taken_ms,
            "id": self.player_id,
        });

        let resp = self.http
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        let body: Value = resp.json().await.unwrap_or_default();
        debug!("Answer response: {:?}", body);
        Ok(body)
    }

    /// Get current game state / leaderboard
    pub async fn get_game_state(&self, game_token: &str) -> Result<Value> {
        let url = format!(
            "{}/games/state?token={}",
            BLOOKET_API, game_token
        );
        let resp = self.http.get(&url).send().await?;
        let body: Value = resp.json().await.unwrap_or_default();
        Ok(body)
    }
}

/// Parse a Blooket question set JSON into typed Question structs
pub fn parse_question_set(data: &Value) -> Vec<Question> {
    let mut questions = Vec::new();

    let qs = match data.get("questions").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return questions,
    };

    for (i, q) in qs.iter().enumerate() {
        let text = q.get("question")
            .or_else(|| q.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            continue;
        }

        let mut answers = Vec::new();
        let mut correct_index = None;

        if let Some(choices) = q.get("answers").and_then(|v| v.as_array()) {
            for (j, choice) in choices.iter().enumerate() {
                let ans_text = choice.get("answer")
                    .or_else(|| choice.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_correct = choice.get("correct")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_correct {
                    correct_index = Some(j);
                }
                answers.push(Answer { index: j, text: ans_text, is_correct });
            }
        }

        let question_type = if answers.len() == 2
            && answers.iter().any(|a| a.text.to_lowercase() == "true")
        {
            QuestionType::TrueFalse
        } else if answers.is_empty() {
            QuestionType::TypeAnswer
        } else {
            QuestionType::MultipleChoice
        };

        questions.push(Question {
            id: q.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("q{}", i))
                .to_string(),
            text,
            answers,
            correct_index,
            question_type,
            time_limit: q.get("timeLimit").and_then(|v| v.as_u64()).map(|v| v as u32),
        });
    }

    questions
}
