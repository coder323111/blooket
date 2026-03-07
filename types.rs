// types.rs — Core data structures for Blooket Engine

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlooketConfig {
    pub game_pin: String,
    pub name: String,
    pub mode: AutoMode,
    pub delay_ms: u64,
    pub llm_endpoint: Option<String>,
    pub llm_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AutoMode {
    /// Fully autonomous — answer all questions automatically via LLM
    Auto,
    /// Hint mode — show correct answer to human player
    Hint,
    /// Speed mode — answer as fast as possible (0 delay)
    Speed,
}

impl std::fmt::Display for AutoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutoMode::Auto => write!(f, "auto"),
            AutoMode::Hint => write!(f, "hint"),
            AutoMode::Speed => write!(f, "speed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub answers: Vec<Answer>,
    pub correct_index: Option<usize>,
    pub question_type: QuestionType,
    pub time_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub index: usize,
    pub text: String,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuestionType {
    MultipleChoice,
    TrueFalse,
    TypeAnswer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub pin: String,
    pub player_name: String,
    pub score: i64,
    pub streak: u32,
    pub questions_answered: u32,
    pub correct: u32,
    pub incorrect: u32,
    pub current_question: Option<Question>,
    pub game_mode: String,
    pub host_id: Option<String>,
}

impl GameState {
    pub fn new(pin: &str, name: &str) -> Self {
        Self {
            pin: pin.to_string(),
            player_name: name.to_string(),
            score: 0,
            streak: 0,
            questions_answered: 0,
            correct: 0,
            incorrect: 0,
            current_question: None,
            game_mode: String::new(),
            host_id: None,
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.questions_answered == 0 {
            return 0.0;
        }
        self.correct as f64 / self.questions_answered as f64 * 100.0
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMRequest {
    pub question: String,
    pub choices: Vec<String>,
    pub question_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMResponse {
    pub answer_index: usize,
    pub answer_text: String,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResult {
    pub pin: String,
    pub name: String,
    pub final_score: i64,
    pub questions_total: u32,
    pub correct: u32,
    pub accuracy: f64,
    pub mode: String,
    pub duration_secs: u64,
}
