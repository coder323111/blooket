// crypto.rs — Blooket authentication and token helpers

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use rand::Rng;

/// Generate a random player ID in Blooket's format
pub fn random_player_id() -> String {
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen_range(100_000_000..999_999_999);
    id.to_string()
}

/// Generate a random game session token
pub fn random_session_token(length: usize) -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .chars()
        .collect();
    (0..length)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

/// Base64-encode a string (Blooket uses this for some payloads)
pub fn b64_encode(input: &str) -> String {
    general_purpose::STANDARD.encode(input.as_bytes())
}

/// Base64-decode a string
pub fn b64_decode(input: &str) -> Result<String> {
    let bytes = general_purpose::STANDARD.decode(input)?;
    Ok(String::from_utf8(bytes)?)
}

/// Build the Blooket join payload
pub fn build_join_payload(pin: &str, name: &str, player_id: &str) -> serde_json::Value {
    serde_json::json!({
        "gamePin": pin,
        "name": name,
        "id": player_id,
        "version": 20,
    })
}

/// Simulate human-like delay with jitter (milliseconds)
pub fn human_delay_ms(base_ms: u64) -> u64 {
    let mut rng = rand::thread_rng();
    let jitter: i64 = rng.gen_range(-200..400);
    let result = base_ms as i64 + jitter;
    result.max(50) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_player_id() {
        let id = random_player_id();
        assert!(id.len() >= 9);
        assert!(id.parse::<u64>().is_ok());
    }

    #[test]
    fn test_b64_roundtrip() {
        let original = "hello blooket";
        let encoded = b64_encode(original);
        let decoded = b64_decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}
