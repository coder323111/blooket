// answer.rs — Answer selection engine for Blooket

use anyhow::Result;
use serde_json::Value;
use tracing::{debug, info};

use crate::types::*;

/// Select the best answer for a question.
/// Priority: known correct > LLM > first answer (fallback)
pub async fn select_answer(
    question: &Question,
    llm_endpoint: Option<&str>,
    llm_api_key: Option<&str>,
) -> Result<(usize, f64)> {
    // 1. If we already know the correct answer (from question set), use it instantly
    if let Some(idx) = question.correct_index {
        info!("✅ Known answer: index {} = '{}'", idx, question.answers[idx].text);
        return Ok((idx, 1.0));
    }

    // 2. Try LLM if configured
    if let (Some(endpoint), Some(api_key)) = (llm_endpoint, llm_api_key) {
        if let Ok((idx, confidence)) = ask_llm(question, endpoint, api_key).await {
            info!("🤖 LLM answer: index {} (confidence {:.0}%)", idx, confidence * 100.0);
            return Ok((idx, confidence));
        }
    }

    // 3. Fallback: pick first answer
    debug!("⚠️  No LLM available, picking first answer");
    Ok((0, 0.25))
}

/// Ask the configured LLM for the correct answer
pub async fn ask_llm(
    question: &Question,
    endpoint: &str,
    api_key: &str,
) -> Result<(usize, f64)> {
    let choices_text: Vec<String> = question.answers
        .iter()
        .enumerate()
        .map(|(i, a)| format!("{}. {}", i, a.text))
        .collect();

    let system = "You are a quiz answering assistant. \
        Given a question and multiple choice answers, respond with ONLY valid JSON: \
        {\"answer_index\": <int>, \"confidence\": <0.0-1.0>, \"reasoning\": \"<brief>\"}. \
        No other text. No markdown.";

    let user = format!(
        "Question: {}\n\nChoices:\n{}\n\nPick the correct answer.",
        question.text,
        choices_text.join("\n")
    );

    let payload = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 256,
        "messages": [
            {"role": "user", "content": format!("{}\n\n{}", system, user)}
        ]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let body: Value = resp.json().await?;
    let text = body["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    // Strip markdown fences if present
    let clean = text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: Value = serde_json::from_str(clean)?;
    let idx = parsed["answer_index"].as_u64().unwrap_or(0) as usize;
    let confidence = parsed["confidence"].as_f64().unwrap_or(0.5);

    // Validate index bounds
    if idx >= question.answers.len() {
        return Err(anyhow::anyhow!("LLM returned out-of-bounds index {}", idx));
    }

    Ok((idx, confidence))
}

/// Format a question for display in hint mode
pub fn format_hint(question: &Question, answer_idx: usize, confidence: f64) -> String {
    let mut lines = vec![
        format!("\n❓ {}", question.text),
        String::new(),
    ];
    for (i, ans) in question.answers.iter().enumerate() {
        let marker = if i == answer_idx { "✅" } else { "  " };
        lines.push(format!("  {} {}. {}", marker, i, ans.text));
    }
    lines.push(String::new());
    lines.push(format!("   Confidence: {:.0}%", confidence * 100.0));
    lines.join("\n")
}
