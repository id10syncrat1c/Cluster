use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::io::{self, Write};
use std::process::Command;
use std::time::Instant;

use crate::config::ProviderConfig;

pub struct StreamResult {
    pub text: String,
    pub estimated_tokens: usize,
    pub elapsed_seconds: f64,
    pub tokens_per_second: f64,
}

pub fn known_providers() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            name: "OpenRouter".into(),
            endpoint: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: "openrouter/free".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
        },
        ProviderConfig {
            name: "OpenAI".into(),
            endpoint: "https://api.openai.com/v1/chat/completions".into(),
            model: "gpt-4o-mini".into(),
            api_key_env: "OPENAI_API_KEY".into(),
        },
        ProviderConfig {
            name: "llama.cpp".into(),
            endpoint: "http://127.0.0.1:8080/v1/chat/completions".into(),
            model: "local-model".into(),
            api_key_env: String::new(),
        },
        ProviderConfig {
            name: "Custom".into(),
            endpoint: "http://127.0.0.1:8080/v1/chat/completions".into(),
            model: "your-model".into(),
            api_key_env: "CLUSTER_API_KEY".into(),
        },
    ]
}

fn api_key(provider: &ProviderConfig) -> Option<String> {
    if provider.api_key_env.trim().is_empty() { return None; }
    std::env::var(&provider.api_key_env).ok().filter(|key| !key.trim().is_empty())
}

fn models_endpoint(chat_endpoint: &str) -> String {
    let endpoint = chat_endpoint.trim_end_matches('/');
    if let Some(prefix) = endpoint.strip_suffix("/chat/completions") {
        format!("{prefix}/models")
    } else {
        format!("{endpoint}/models")
    }
}

pub async fn list_models(client: &Client, provider: &ProviderConfig) -> Result<Vec<String>> {
    let mut request = client.get(models_endpoint(&provider.endpoint));
    if let Some(key) = api_key(provider) { request = request.bearer_auth(key); }

    let response = request.send().await.context("Could not contact provider model catalogue")?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Model catalogue returned {status}: {}", error_detail(&body)));
    }

    let payload: Value = serde_json::from_str(&body).context("Provider returned invalid model JSON")?;
    let models = payload["data"].as_array()
        .or_else(|| payload["models"].as_array())
        .ok_or_else(|| anyhow!("Provider response did not contain a models list"))?;

    let mut ids = models.iter()
        .filter_map(|model| model["id"].as_str().or_else(|| model["name"].as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn error_detail(body: &str) -> String {
    serde_json::from_str::<Value>(body).ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(500).collect())
}

fn terminal_dimensions() -> (u16, u16) {
    if let Ok(output) = Command::new("stty").arg("size").output() {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            let mut parts = value.split_whitespace().filter_map(|part| part.parse::<u16>().ok());
            if let (Some(rows), Some(columns)) = (parts.next(), parts.next()) {
                return (columns, rows);
            }
        }
    }
    let columns = std::env::var("COLUMNS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let rows = std::env::var("LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    (columns, rows)
}

fn render_gauge(stdout: &mut impl Write, width: u16, height: u16, spinner: &str, tokens: usize, elapsed: f64) -> io::Result<()> {
    if width < 30 || height < 3 { return Ok(()); }
    let speed = if elapsed > 0.0 { tokens as f64 / elapsed } else { 0.0 };
    let label = format!("{spinner} Thinking · ~{speed:.1} tok/s");
    let visible = label.chars().take(width.saturating_sub(1) as usize).collect::<String>();
    let x = width.saturating_sub(visible.chars().count() as u16) + 1;
    write!(stdout, "\x1b[s\x1b[{};{}H\x1b[2K\x1b[2m{visible}\x1b[0m\x1b[u", height, x)?;
    stdout.flush()
}

pub async fn stream_message(
    client: &Client,
    provider: &ProviderConfig,
    model: &str,
    messages: &[Value],
    show_activity: bool,
) -> Result<StreamResult> {
    let mut request = client.post(&provider.endpoint).json(&json!({
        "model": model,
        "messages": messages,
        "stream": true
    }));
    if let Some(key) = api_key(provider) { request = request.bearer_auth(key); }
    if provider.name == "OpenRouter" {
        request = request.header("X-Title", "Cluster - Libra");
    }

    let response = request.send().await.context("Failed to reach AI provider")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Provider returned {status}: {}", error_detail(&body)));
    }

    let mut stream = response.bytes_stream();
    let mut pending = String::new();
    let mut output = String::new();
    let started = Instant::now();
    let mut spinner_index = 0usize;
    let spinner = ["◐", "◓", "◑", "◒"];
    let (width, height) = terminal_dimensions();
    let mut stdout = io::stdout();

    while let Some(chunk) = stream.next().await {
        pending.push_str(&String::from_utf8_lossy(&chunk.context("Connection interrupted while streaming response")?));
        while let Some(newline) = pending.find('\n') {
            let line = pending[..newline].trim().to_owned();
            pending.drain(..=newline);
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" { break; }

            let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
            let content = &event["choices"][0]["delta"]["content"];
            if let Some(text) = content.as_str() {
                if !text.is_empty() {
                    print!("{text}");
                    io::stdout().flush()?;
                    output.push_str(text);
                }
            } else if let Some(parts) = content.as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str() {
                        print!("{text}");
                        io::stdout().flush()?;
                        output.push_str(text);
                    }
                }
            }

            if show_activity && !output.is_empty() {
                let tokens = output.chars().count().div_ceil(4);
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                render_gauge(&mut stdout, width, height, spinner[spinner_index % spinner.len()], tokens, elapsed)?;
                spinner_index += 1;
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    if output.trim().is_empty() {
        return Err(anyhow!("The provider returned no readable response"));
    }
    if show_activity && height >= 3 {
        write!(stdout, "\x1b[s\x1b[{};1H\x1b[2K\x1b[u", height)?;
        stdout.flush()?;
    }

    let estimated_tokens = output.chars().count().div_ceil(4);
    Ok(StreamResult {
        text: output,
        estimated_tokens,
        elapsed_seconds: elapsed,
        tokens_per_second: if elapsed > 0.0 { estimated_tokens as f64 / elapsed } else { 0.0 },
    })
}
