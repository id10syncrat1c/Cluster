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
        .user_agent("Cluster/0.3.0")
        .build()?;

    let mut memory = MemoryStore::load()?;

    let mut messages = vec![json!({
        "role": "system",
        "content": SYSTEM_PROMPT
    })];

    for turn in &memory.data.history {
        messages.push(json!({
            "role": "user",
            "content": turn.user_message
        }));
        messages.push(json!({
            "role": "assistant",
            "content": turn.assistant_response
        }));
    }

    println!();
    println!("  Cluster — Libra v0.3.0");
    println!("  Model: {model}");
    println!("  Memory: {} durable memories, {} conversation turns", memory.data.durable.len(), memory.data.history.len());
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
            println!("/clear            Clear conversation history");
            println!("/memory           Show memory status");
            println!("/model            Show current model");
            println!("/model <name>     Change model");
            println!("/exit             Exit Cluster");
            println!("/memory export    Export memory to file");
            println!("/memory reset     Reset all memories");
            println!("/memory inspect   Show memory inspection");
            continue;
        }

        if input == "/clear" {
            memory.data.history.clear();
            memory.save()?;
            messages.truncate(1);
            println!("Conversation history cleared.");
            continue;
        }

        if input.starts_with("/memory ") {
            let subcommand = input.strip_prefix("/memory ").unwrap_or("").trim();

            match subcommand {
                "export" => {
                    let default_path = memory.path().parent().unwrap_or(std::path::Path::new(".")).join("exported_memory.json");
                    println!("Exporting memory to file. Default path: {}", default_path.display());
                    memory.export(&default_path)?;
                    println!("Memory exported to {}", default_path.display());
                }
                "reset" => {
                    memory.reset()?;
                    messages.truncate(1);
                    println!("All memories reset.");
                }
                "inspect" => {
                    println!("Durable memories: {}", memory.data.durable.len());
                    println!("Conversation turns: {}", memory.data.history.len());
                    if !memory.data.durable.is_empty() {
                        println!("\nRecent durable memories:");
                        for mem in memory.data.durable.iter().take(5) {
                            println!("  [{}] {}...", &mem.content[..std::cmp::min(60, mem.content.len())], mem.category.as_deref().unwrap_or(""));
                        }
                    }
                    if !memory.data.history.is_empty() {
                        println!("\nRecent conversation turns:");
                        for turn in memory.data.history.iter().take(3) {
                            let user_preview = &turn.user_message[..std::cmp::min(50, turn.user_message.len())];
                            let resp_preview = &turn.assistant_response[..std::cmp::min(50, turn.assistant_response.len())];
                            println!("  User: {}... Assistant: {}...", user_preview, resp_preview);
                        }
                    }
                }
                _ => {
                    println!("Unknown memory subcommand. Use: export, reset, inspect");
                }
            }
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

        let context_msgs = memory.get_relevant_context(&input);
        for msg in context_msgs {
            messages.push(msg);
        }

        print!("\nCluster is thinking...\n");
        io::stdout().flush()?;

        match send_message(&client, api_key, &model, &messages[1..]).await {
            Ok(reply) => {
                messages.push(json!({
                    "role": "assistant",
                    "content": reply
                }));

                memory.add_turn(input.to_string(), reply.clone())?;

                if messages.len() > MAX_HISTORY + 1 {
                    let excess = messages.len() - MAX_HISTORY - 1;
                    messages.drain(1..1 + excess);
                }

                memory.save()?;

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