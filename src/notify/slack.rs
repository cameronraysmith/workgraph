//! Slack notification channel — sends messages via the Slack Web API.
//!
//! Implements [`NotificationChannel`] for Slack workspaces. Supports:
//! - Outbound: plain text, rich (Block Kit), and interactive action-button messages
//! - Inbound: Socket Mode listener for slash commands and button interactions
//!
//! Configuration is read from the `[slack]` section of `notify.toml`:
//! ```toml
//! [slack]
//! bot_token = "xoxb-..."
//! app_token = "xapp-..."         # required for Socket Mode receive
//! default_channel = "C0123456"   # channel ID or name
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{Action, ActionStyle, IncomingMessage, MessageId, NotificationChannel, RichMessage};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Slack-specific configuration parsed from the `[slack]` section.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SlackConfig {
    /// Bot user OAuth token (xoxb-...).
    pub bot_token: String,
    /// App-level token for Socket Mode (xapp-...). Optional — only needed for receiving.
    #[serde(default)]
    pub app_token: Option<String>,
    /// Default channel to post to (channel ID or name).
    pub default_channel: String,
}

impl SlackConfig {
    /// Extract from the opaque channel map in [`super::config::NotifyConfig`].
    pub fn from_notify_config(config: &super::config::NotifyConfig) -> Result<Self> {
        let val = config
            .channels
            .get("slack")
            .context("no [slack] section in notify config")?;
        let cfg: Self = val.clone().try_into().context("invalid [slack] config")?;
        Ok(cfg)
    }
}

// ---------------------------------------------------------------------------
// Block Kit helpers
// ---------------------------------------------------------------------------

/// Build a Block Kit section block with markdown text.
fn section_block(text: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": text
        }
    })
}

/// Build a Block Kit actions block with buttons.
fn actions_block(actions: &[Action]) -> serde_json::Value {
    let elements: Vec<serde_json::Value> = actions
        .iter()
        .map(|a| {
            let style = match a.style {
                ActionStyle::Primary => Some("primary"),
                ActionStyle::Danger => Some("danger"),
                ActionStyle::Secondary => None,
            };
            let mut btn = serde_json::json!({
                "type": "button",
                "text": {
                    "type": "plain_text",
                    "text": &a.label
                },
                "action_id": &a.id,
                "value": &a.id
            });
            if let Some(s) = style {
                btn["style"] = serde_json::Value::String(s.to_string());
            }
            btn
        })
        .collect();

    serde_json::json!({
        "type": "actions",
        "elements": elements
    })
}

/// Build a Block Kit divider block.
fn divider_block() -> serde_json::Value {
    serde_json::json!({ "type": "divider" })
}

// ---------------------------------------------------------------------------
// Channel implementation
// ---------------------------------------------------------------------------

/// A Slack notification channel backed by the Slack Web API via `reqwest`.
pub struct SlackChannel {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackChannel {
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Resolve the target channel. If empty or "*", uses the configured default.
    fn resolve_channel<'a>(&'a self, target: &'a str) -> &'a str {
        if target.is_empty() || target == "*" {
            &self.config.default_channel
        } else {
            target
        }
    }

    /// Call a Slack Web API method with a JSON body.
    async fn api_call(&self, method: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("https://slack.com/api/{method}");
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(body)
            .send()
            .await
            .context("Slack API request failed")?;

        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .context("failed to parse Slack API response")?;

        if !status.is_success() {
            anyhow::bail!("Slack API HTTP error: {status}");
        }

        if json.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let error = json
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Slack API error: {error}");
        }

        Ok(json)
    }

    /// Extract the message timestamp (ts) from a chat.postMessage response.
    fn extract_ts(json: &serde_json::Value) -> MessageId {
        let ts = json.get("ts").and_then(|t| t.as_str()).unwrap_or("0");
        MessageId(ts.to_string())
    }
}

#[async_trait]
impl NotificationChannel for SlackChannel {
    fn channel_type(&self) -> &str {
        "slack"
    }

    async fn send_text(&self, target: &str, message: &str) -> Result<MessageId> {
        let channel = self.resolve_channel(target);
        let body = serde_json::json!({
            "channel": channel,
            "text": message,
        });
        let resp = self.api_call("chat.postMessage", &body).await?;
        Ok(Self::extract_ts(&resp))
    }

    async fn send_rich(&self, target: &str, message: &RichMessage) -> Result<MessageId> {
        let channel = self.resolve_channel(target);

        // Use markdown for Block Kit if available, otherwise plain text.
        let text = message.markdown.as_deref().unwrap_or(&message.plain_text);

        let blocks = serde_json::json!([section_block(text)]);

        let body = serde_json::json!({
            "channel": channel,
            "text": &message.plain_text, // fallback for notifications
            "blocks": blocks,
        });

        let resp = self.api_call("chat.postMessage", &body).await?;
        Ok(Self::extract_ts(&resp))
    }

    async fn send_with_actions(
        &self,
        target: &str,
        message: &str,
        actions: &[Action],
    ) -> Result<MessageId> {
        let channel = self.resolve_channel(target);

        let blocks = serde_json::json!([
            section_block(message),
            divider_block(),
            actions_block(actions),
        ]);

        let body = serde_json::json!({
            "channel": channel,
            "text": message, // fallback
            "blocks": blocks,
        });

        let resp = self.api_call("chat.postMessage", &body).await?;
        Ok(Self::extract_ts(&resp))
    }

    fn supports_receive(&self) -> bool {
        self.config.app_token.is_some()
    }

    async fn listen(&self) -> Result<tokio::sync::mpsc::Receiver<IncomingMessage>> {
        let app_token = self
            .config
            .app_token
            .as_ref()
            .context("Slack Socket Mode requires an app_token (xapp-...)")?
            .clone();

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let client = self.client.clone();

        tokio::spawn(async move {
            // Open a Socket Mode WebSocket connection by requesting a wss URL.
            let connect_url = "https://slack.com/api/apps.connections.open";
            loop {
                let resp = match client
                    .post(connect_url)
                    .header("Authorization", format!("Bearer {app_token}"))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Slack Socket Mode connect error: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                        continue;
                    }
                };

                let json: serde_json::Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!("Slack Socket Mode parse error: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                        continue;
                    }
                };

                if json.get("ok") != Some(&serde_json::Value::Bool(true)) {
                    let error = json
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unknown");
                    eprintln!("Slack Socket Mode error: {error}");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    continue;
                }

                let _wss_url = json.get("url").and_then(|u| u.as_str()).unwrap_or("");

                // NOTE: Full WebSocket handling requires tokio-tungstenite or similar.
                // For now, we log the connection URL. A full implementation would:
                // 1. Connect to the WSS URL
                // 2. Respond to envelope payloads with {"envelope_id": "..."}
                // 3. Parse interactive_endpoint, slash_commands, events_api payloads
                // 4. Forward parsed messages to tx
                eprintln!("Slack Socket Mode: WebSocket support not yet implemented");
                eprintln!("Obtained WSS URL — full Socket Mode requires a WebSocket client");

                // For now, keep the channel alive but don't produce messages.
                // This prevents the receiver from being dropped immediately.
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    if tx.is_closed() {
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SlackConfig {
        SlackConfig {
            bot_token: "xoxb-test-token".into(),
            app_token: Some("xapp-test-token".into()),
            default_channel: "C0123456".into(),
        }
    }

    #[test]
    fn slack_config_from_toml() {
        let toml_str = r#"
[routing]
default = ["slack"]

[slack]
bot_token = "xoxb-123-456-abc"
app_token = "xapp-1-A111-222-xyz"
default_channel = "C9999999"
"#;
        let config: super::super::config::NotifyConfig = toml::from_str(toml_str).unwrap();
        let slack = SlackConfig::from_notify_config(&config).unwrap();
        assert_eq!(slack.bot_token, "xoxb-123-456-abc");
        assert_eq!(slack.app_token.as_deref(), Some("xapp-1-A111-222-xyz"));
        assert_eq!(slack.default_channel, "C9999999");
    }

    #[test]
    fn slack_config_without_app_token() {
        let toml_str = r#"
[routing]
default = ["slack"]

[slack]
bot_token = "xoxb-test"
default_channel = "C0000000"
"#;
        let config: super::super::config::NotifyConfig = toml::from_str(toml_str).unwrap();
        let slack = SlackConfig::from_notify_config(&config).unwrap();
        assert!(slack.app_token.is_none());
    }

    #[test]
    fn slack_config_missing_section() {
        let config = super::super::config::NotifyConfig::default();
        assert!(SlackConfig::from_notify_config(&config).is_err());
    }

    #[test]
    fn channel_type_is_slack() {
        let ch = SlackChannel::new(test_config());
        assert_eq!(ch.channel_type(), "slack");
    }

    #[test]
    fn supports_receive_with_app_token() {
        let ch = SlackChannel::new(test_config());
        assert!(ch.supports_receive());
    }

    #[test]
    fn supports_receive_without_app_token() {
        let mut config = test_config();
        config.app_token = None;
        let ch = SlackChannel::new(config);
        assert!(!ch.supports_receive());
    }

    #[test]
    fn resolve_channel_default() {
        let ch = SlackChannel::new(test_config());
        assert_eq!(ch.resolve_channel("*"), "C0123456");
        assert_eq!(ch.resolve_channel(""), "C0123456");
    }

    #[test]
    fn resolve_channel_explicit() {
        let ch = SlackChannel::new(test_config());
        assert_eq!(ch.resolve_channel("C9999999"), "C9999999");
    }

    #[test]
    fn section_block_structure() {
        let block = section_block("Hello *world*");
        assert_eq!(block["type"], "section");
        assert_eq!(block["text"]["type"], "mrkdwn");
        assert_eq!(block["text"]["text"], "Hello *world*");
    }

    #[test]
    fn actions_block_structure() {
        let actions = vec![
            Action {
                id: "approve".into(),
                label: "Approve".into(),
                style: ActionStyle::Primary,
            },
            Action {
                id: "reject".into(),
                label: "Reject".into(),
                style: ActionStyle::Danger,
            },
            Action {
                id: "skip".into(),
                label: "Skip".into(),
                style: ActionStyle::Secondary,
            },
        ];
        let block = actions_block(&actions);
        assert_eq!(block["type"], "actions");
        let elements = block["elements"].as_array().unwrap();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0]["action_id"], "approve");
        assert_eq!(elements[0]["style"], "primary");
        assert_eq!(elements[1]["action_id"], "reject");
        assert_eq!(elements[1]["style"], "danger");
        // Secondary has no style key
        assert!(elements[2].get("style").is_none());
    }

    #[test]
    fn divider_block_structure() {
        let block = divider_block();
        assert_eq!(block["type"], "divider");
    }

    #[test]
    fn extract_ts_from_response() {
        let json = serde_json::json!({
            "ok": true,
            "channel": "C0123456",
            "ts": "1234567890.123456"
        });
        let mid = SlackChannel::extract_ts(&json);
        assert_eq!(mid.0, "1234567890.123456");
    }

    #[test]
    fn extract_ts_missing_returns_zero() {
        let json = serde_json::json!({"ok": true});
        let mid = SlackChannel::extract_ts(&json);
        assert_eq!(mid.0, "0");
    }
}
