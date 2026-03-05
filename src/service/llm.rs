//! Lightweight LLM dispatch for internal workgraph calls (triage, checkpoint, etc.).
//!
//! Resolves model + provider via `resolve_model_for_role()` and dispatches to either:
//! - Claude CLI (`claude --model X --print --dangerously-skip-permissions PROMPT`)
//! - Native Anthropic API client (when provider is "anthropic" and native executor is configured)
//! - Native OpenAI-compatible API client (when provider is "openai"/"openrouter")

use std::process;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::{Config, DispatchRole};

/// Run a lightweight (no tool-use) LLM call for an internal dispatch role.
///
/// Resolves the model and provider for the given role, then dispatches via:
/// 1. If `provider` is set to a native provider ("anthropic", "openai", "openrouter"),
///    attempts a direct API call using the native client.
/// 2. Falls back to shelling out to `claude` CLI.
pub fn run_lightweight_llm_call(
    config: &Config,
    role: DispatchRole,
    prompt: &str,
    timeout_secs: u64,
) -> Result<String> {
    let resolved = config.resolve_model_for_role(role);
    let model = &resolved.model;
    let provider = resolved.provider.as_deref();

    // Try native API call if provider is explicitly configured
    if let Some(prov) = provider {
        match prov {
            "anthropic" => {
                if let Ok(result) = call_anthropic_native(model, prompt, timeout_secs) {
                    return Ok(result);
                }
            }
            "openai" | "openrouter" => {
                if let Ok(result) = call_openai_native(model, prompt, timeout_secs) {
                    return Ok(result);
                }
            }
            _ => {}
        }
    }

    call_claude_cli(model, prompt, timeout_secs)
}

fn call_claude_cli(model: &str, prompt: &str, timeout_secs: u64) -> Result<String> {
    let output = process::Command::new("timeout")
        .arg(format!("{}s", timeout_secs))
        .arg("claude")
        .arg("--model")
        .arg(model)
        .arg("--print")
        .arg("--dangerously-skip-permissions")
        .arg(prompt)
        .output()
        .context("Failed to run claude CLI for lightweight LLM call")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Claude CLI call failed (exit {:?}): {}",
            output.status.code(),
            stderr.chars().take(200).collect::<String>()
        );
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.is_empty() {
        anyhow::bail!("Empty response from claude CLI");
    }
    Ok(result)
}

fn call_anthropic_native(model: &str, prompt: &str, timeout_secs: u64) -> Result<String> {
    use crate::executor::native::client::{
        AnthropicClient, ContentBlock, LlmClient, Message, MessagesRequest, Role,
    };

    let client = AnthropicClient::from_env(model)
        .context("Failed to create Anthropic client for lightweight call")?;

    let request = MessagesRequest {
        model: model.to_string(),
        max_tokens: 1024,
        system: None,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: prompt.to_string(),
            }],
        }],
        tools: vec![],
        stream: false,
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    let response = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(timeout_secs), client.send(&request))
            .await
            .context("Native Anthropic call timed out")?
    })?;

    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    let text = text.trim().to_string();
    if text.is_empty() {
        anyhow::bail!("Empty response from native Anthropic call");
    }
    Ok(text)
}

fn call_openai_native(model: &str, prompt: &str, timeout_secs: u64) -> Result<String> {
    use crate::executor::native::client::{
        ContentBlock, LlmClient, Message, MessagesRequest, Role,
    };
    use crate::executor::native::openai_client::OpenAiClient;

    let client = OpenAiClient::from_env(model)
        .context("Failed to create OpenAI client for lightweight call")?;

    let request = MessagesRequest {
        model: model.to_string(),
        max_tokens: 1024,
        system: None,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: prompt.to_string(),
            }],
        }],
        tools: vec![],
        stream: false,
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    let response = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(timeout_secs), client.send(&request))
            .await
            .context("Native OpenAI call timed out")?
    })?;

    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    let text = text.trim().to_string();
    if text.is_empty() {
        anyhow::bail!("Empty response from native OpenAI call");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, DispatchRole};

    #[test]
    fn test_lightweight_llm_dispatch_resolves_model() {
        let config = Config::default();
        let resolved = config.resolve_model_for_role(DispatchRole::Triage);
        assert_eq!(resolved.model, "haiku");
        assert!(
            resolved.provider.is_none(),
            "Default triage should have no explicit provider"
        );
    }

    #[test]
    fn test_lightweight_llm_dispatch_with_provider_override() {
        let mut config = Config::default();
        config.models.set_model(DispatchRole::Triage, "gpt-4o-mini");
        config.models.set_provider(DispatchRole::Triage, "openai");

        let resolved = config.resolve_model_for_role(DispatchRole::Triage);
        assert_eq!(resolved.model, "gpt-4o-mini");
        assert_eq!(resolved.provider, Some("openai".to_string()));
    }
}
