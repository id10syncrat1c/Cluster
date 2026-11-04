mod memory;

use anyhow::{anyhow, Context, Result};
use memory::MemoryStore;
use reqwest::Client;
use serde_json::{json, Value};
use std::io::{self, Write};

const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL: &str = "openrouter/free";
const MAX_HISTORY: usize = 40;

const SYSTEM_PROMPT: &str = r#"You are Cluster, a personal AI agent.
Your current release is Libra.
Be helpful, direct, technically capable, and honest.
You do not currently have tool execution capabilities.
Never claim to have performed actions you have not performed.
You may use previous conversation messages as context."#;

async fn send_message(
    client: &Client,
    api_key: &str,
    model: &str,
    history: &[Value],
) -> Result<String> {
    let mut messages = vec![json!({
        "role": "system",
        "content": SYSTEM_PROMPT
    })];

    messages.extend_from_slice(history);

    let response = client
        .post(API_URL)
        .bearer_auth(api_key)
        .header("X-Title", "Cluster - Libra")
        .json(&json!({
            "model": model,
            "messages": messages,
            "stream": false
        }))
        .send()
        .await
        .context("Failed to reach OpenRouter")?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        let detail = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(String::from))
            .unwrap_or(body);

        return Err(anyhow!("OpenRouter returned {status}: {detail}"));
    }

    let data: Value = serde_json::from_str(&body)
        .context("OpenRouter returned invalid JSON")?;

    let content = &data["choices"][0]["message"]["content"];

    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }

    if let Some(parts) = content.as_array() {
        let text = parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("");

        if !text.is_empty() {
            return Ok(text);
        }
    }

    Err(anyhow!("The model returned no readable response"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY is not set")?;

    let api_key = api_key.trim();

    if api_key.is_empty() {
        return Err(anyhow!("OPENROUTER_API_KEY is empty"));
    }

    let mut model = std::env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let client = Client::builder()
        .user_agent("Cluster/0.2.0")
        .build()?;

    let mut memory = MemoryStore::load()?;

    let mut messages = vec![json!({
        "role": "system",
        "content": SYSTEM_PROMPT
    })];

    messages.extend(memory.messages.iter().cloned());

    println!();
    println!("  Cluster — Libra v0.2.0");
    println!("  Model: {model}");
    println!("  Memory: {} messages restored", memory.messages.len());
    println!("  Type /help for commands.");
    println!();

    loop {
        print!("libra > ");
        io::stdout().flush()?;

        let mut input = String::new();

        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "/exit" || input == "/quit" {
            println!("Cluster signing off.");
            break;
        }

        if input == "/help" {
            println!("/help             Show available commands");
            println!("/clear            Clear conversation memory");
            println!("/memory           Show memory status");
            println!("/model            Show current model");
            println!("/model <name>     Change model");
            println!("/exit             Exit Cluster");
            continue;
        }

        if input == "/clear" {
            memory.clear()?;
            messages.truncate(1);
            println!("Conversation memory cleared.");
            continue;
        }

        if input == "/memory" {
            println!("Stored messages: {}", memory.messages.len());
            println!("Memory file: {}", memory.path().display());
            continue;
        }

        if input == "/model" {
            println!("Current model: {model}");
            continue;
        }

        if let Some(new_model) = input.strip_prefix("/model ") {
            let new_model = new_model.trim();

            if !new_model.is_empty() {
                model = new_model.to_string();
                println!("Model changed to: {model}");
            }
            continue;
        }

        messages.push(json!({
            "role": "user",
            "content": input
        }));

        print!("\nCluster is thinking...\n");
        io::stdout().flush()?;

        match send_message(&client, api_key, &model, &messages[1..]).await {
            Ok(reply) => {
                messages.push(json!({
                    "role": "assistant",
                    "content": reply
                }));

                if messages.len() > MAX_HISTORY + 1 {
                    let excess = messages.len() - MAX_HISTORY - 1;
                    messages.drain(1..1 + excess);
                }

                memory.messages = messages[1..].to_vec();

                if let Err(error) = memory.save() {
                    eprintln!("\nMemory save failed: {error}");
                }

                println!("\n{}\n", reply);
            }
            Err(error) => {
                messages.pop();
                eprintln!("\nRequest failed: {error}\n");
            }
        }
    }

    Ok(())
}
