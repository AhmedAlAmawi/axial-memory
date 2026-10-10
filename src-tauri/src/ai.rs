//! The one network call the app makes on her behalf: sending one definition's
//! extract to Claude or ChatGPT with her own key, when she asks.

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    Openai,
}

impl Provider {
    pub fn detect(key: &str) -> Option<Provider> {
        let k = key.trim();
        if k.starts_with("sk-ant-") {
            Some(Provider::Anthropic)
        } else if k.starts_with("sk-") {
            Some(Provider::Openai)
        } else {
            None
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude",
            Provider::Openai => "ChatGPT",
        }
    }
}

const ANTHROPIC: &str = "https://api.anthropic.com/v1";
const OPENAI: &str = "https://api.openai.com/v1";

fn friendly(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.pointer("/error/message").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    match status.as_u16() {
        401 | 403 => "That AI key was refused. Check it was copied whole, or make a new one.".into(),
        402 | 429 => format!("The AI service is busy or the account is out of credit. {detail}").trim().to_string(),
        _ => format!("The AI service answered {status}. {detail}").trim().to_string(),
    }
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value, String> {
    let res =
        req.send().await.map_err(|e| format!("Couldn't reach the AI service ({e}). Check the internet connection."))?;
    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(friendly(status, &body));
    }
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

/// Checks the key works and picks the provider's current general model, so a
/// retired model name never breaks the app.
pub async fn choose_model(http: &reqwest::Client, provider: Provider, key: &str) -> Result<String, String> {
    match provider {
        Provider::Anthropic => {
            let v = send(
                http.get(format!("{ANTHROPIC}/models?limit=100"))
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01"),
            )
            .await?;
            let ids: Vec<String> = v["data"]
                .as_array()
                .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            ids.iter()
                .find(|id| id.contains("sonnet"))
                .or(ids.first())
                .cloned()
                .ok_or_else(|| "This key can't use any Claude models.".to_string())
        }
        Provider::Openai => {
            let v = send(http.get(format!("{OPENAI}/models")).bearer_auth(key)).await?;
            let mut models: Vec<(String, i64)> = v["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| Some((m["id"].as_str()?.to_string(), m["created"].as_i64().unwrap_or(0))))
                        .collect()
                })
                .unwrap_or_default();
            models.sort_by_key(|m| std::cmp::Reverse(m.1));
            let flagship = |id: &str| {
                id.strip_prefix("gpt-")
                    .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.') && !rest.is_empty())
            };
            let general = |id: &str| {
                id.starts_with("gpt-")
                    && ![
                        "mini",
                        "nano",
                        "audio",
                        "realtime",
                        "search",
                        "transcribe",
                        "tts",
                        "image",
                        "codex",
                        "instruct",
                    ]
                    .iter()
                    .any(|w| id.contains(w))
            };
            models
                .iter()
                .find(|(id, _)| flagship(id))
                .or_else(|| models.iter().find(|(id, _)| general(id)))
                .map(|(id, _)| id.clone())
                .ok_or_else(|| "This key can't use any ChatGPT models.".to_string())
        }
    }
}

pub async fn complete(
    http: &reqwest::Client,
    provider: Provider,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    match provider {
        Provider::Anthropic => {
            let v = send(
                http.post(format!("{ANTHROPIC}/messages"))
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&json!({
                        "model": model,
                        "max_tokens": 1800,
                        "system": system,
                        "messages": [{ "role": "user", "content": user }],
                    })),
            )
            .await?;
            let text: String = v["content"]
                .as_array()
                .map(|blocks| blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            if text.trim().is_empty() {
                Err("Claude sent back an empty answer. Try again.".into())
            } else {
                Ok(text)
            }
        }
        Provider::Openai => {
            let v = send(http.post(format!("{OPENAI}/chat/completions")).bearer_auth(key).json(&json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user },
                ],
            })))
            .await?;
            v.pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| "ChatGPT sent back an empty answer. Try again.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_providers_from_keys() {
        assert_eq!(Provider::detect("sk-ant-api03-abc"), Some(Provider::Anthropic));
        assert_eq!(Provider::detect(" sk-proj-abc "), Some(Provider::Openai));
        assert_eq!(Provider::detect("AIzaSy-google"), None);
    }
}
