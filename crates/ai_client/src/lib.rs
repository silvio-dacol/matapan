use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Url;
use serde::{Deserialize, Serialize};

/// Configuration for talking to a local Ollama server.
///
/// This crate intentionally only supports Ollama's local HTTP API.
/// It refuses to run if the configured base URL is not local.
#[derive(Debug, Clone)]
pub struct OllamaClientConfig {
    pub base_url: String,
    pub model: String,
}

impl OllamaClientConfig {
    /// Loads config from env vars:
    /// - `OLLAMA_BASE_URL` (default: `http://localhost:11434`)
    /// - `OLLAMA_MODEL`    (default: `llama3.2`)
    pub fn from_env() -> Self {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
        Self { base_url, model }
    }
}

/// Backwards compatible name.
#[deprecated(note = "Use OllamaClientConfig (this crate is Ollama-only)")]
pub type AiClientConfig = OllamaClientConfig;

/// Minimal Ollama chat client (blocking HTTP).
#[derive(Debug, Clone)]
pub struct OllamaClient {
    http: Client,
    base_url: Url,
    model: String,
}

impl OllamaClient {
    pub fn new(config: OllamaClientConfig) -> Result<Self> {
        let base_url = validate_local_base_url(&config.base_url)?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url,
            model: config.model,
        })
    }

    /// Generic helper for a single-turn chat call.
    pub fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let endpoint = self
            .base_url
            .join("api/chat")
            .context("Failed to build Ollama /api/chat URL")?;

        let request = OllamaChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            options: Some(OllamaOptions {
                temperature: Some(0.0),
            }),
        };

        let response: OllamaChatResponse = self
            .http
            .post(endpoint.clone())
            .json(&request)
            .send()
            .with_context(|| format!("POST {endpoint} failed"))?
            .error_for_status()
            .with_context(|| format!("POST {endpoint} returned non-success status"))?
            .json()
            .with_context(|| format!("Failed to parse JSON response from {endpoint}"))?;

        let content = response
            .message
            .map(|m| m.content)
            .ok_or_else(|| anyhow!("Ollama response had no message content"))?;

        Ok(content.trim().to_string())
    }

    /// Translates a piece of text into `target_language`.
    ///
    /// Returns only the translated text (no extra commentary).
    pub fn translate_text(&self, text: &str, target_language: &str) -> Result<String> {
        let system_prompt = format!(
            "You are a translation engine. Translate the user's text to {target_language}. Return only the translated text and nothing else."
        );

        self.chat(&system_prompt, text)
    }
}

/// Backwards compatible name.
#[deprecated(note = "Use OllamaClient (this crate is Ollama-only)")]
pub type OpenAiCompatibleClient = OllamaClient;

fn validate_local_base_url(base_url: &str) -> Result<Url> {
    let url =
        Url::parse(base_url).with_context(|| format!("Invalid OLLAMA_BASE_URL: {base_url}"))?;

    match url.scheme() {
        "http" => {}
        other => {
            return Err(anyhow!(
                "Unsupported scheme '{other}' for OLLAMA_BASE_URL (use http://localhost:11434)"
            ))
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("OLLAMA_BASE_URL is missing a host"))?;

    let is_local = host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1";

    if !is_local {
        return Err(anyhow!(
            "Refusing non-local OLLAMA_BASE_URL host '{host}'. This project only uses local Ollama (use http://localhost:11434)."
        ));
    }

    Ok(url)
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<Message>,
}
