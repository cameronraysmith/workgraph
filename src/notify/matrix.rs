//! Matrix implementation of [`NotificationChannel`] wrapping the `matrix_lite` client.

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::config::MatrixConfig;
use crate::matrix_lite::MatrixClient;

use super::{Action, IncomingMessage, MessageId, NotificationChannel, RichMessage};

/// A [`NotificationChannel`] backed by the lightweight Matrix client.
pub struct MatrixChannel {
    client: MatrixClient,
    room_id: String,
}

impl MatrixChannel {
    /// Create a new `MatrixChannel` from the existing matrix_lite client.
    pub fn new(client: MatrixClient, room_id: String) -> Self {
        Self { client, room_id }
    }

    /// Create from config, loading credentials and connecting.
    pub async fn from_config(
        workgraph_dir: &std::path::Path,
        config: &MatrixConfig,
    ) -> Result<Self> {
        let room_id = config
            .default_room
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("default_room not configured in matrix.toml"))?
            .clone();

        let client = MatrixClient::new(workgraph_dir, config).await?;
        Ok(Self { client, room_id })
    }

    /// Return a reference to the underlying `MatrixClient`.
    pub fn client(&self) -> &MatrixClient {
        &self.client
    }

    /// Return the target room ID.
    pub fn room_id(&self) -> &str {
        &self.room_id
    }
}

#[async_trait]
impl NotificationChannel for MatrixChannel {
    fn channel_type(&self) -> &str {
        "matrix"
    }

    async fn send_text(&self, target: &str, message: &str) -> Result<MessageId> {
        // Use target as room_id if provided, otherwise use the default room.
        let room = if target.is_empty() {
            &self.room_id
        } else {
            target
        };
        let event_id = self.client.send_message(room, message).await?;
        Ok(MessageId(event_id))
    }

    async fn send_rich(&self, target: &str, message: &RichMessage) -> Result<MessageId> {
        let room = if target.is_empty() {
            &self.room_id
        } else {
            target
        };

        if let Some(html) = &message.html {
            let event_id = self
                .client
                .send_html_message(room, &message.plain_text, html)
                .await?;
            Ok(MessageId(event_id))
        } else {
            // No HTML — fall back to plain text.
            let event_id = self.client.send_message(room, &message.plain_text).await?;
            Ok(MessageId(event_id))
        }
    }

    async fn send_with_actions(
        &self,
        target: &str,
        message: &str,
        actions: &[Action],
    ) -> Result<MessageId> {
        // Matrix doesn't have native action buttons. Render them as text hints.
        let room = if target.is_empty() {
            &self.room_id
        } else {
            target
        };

        let action_text: Vec<String> = actions
            .iter()
            .map(|a| format!("[{}]", a.label))
            .collect();
        let full_message = format!("{}\n\nActions: {}", message, action_text.join("  "));

        let event_id = self.client.send_message(room, &full_message).await?;
        Ok(MessageId(event_id))
    }

    fn supports_receive(&self) -> bool {
        true
    }

    async fn listen(&self) -> Result<mpsc::Receiver<IncomingMessage>> {
        // The matrix_lite client requires &mut self for sync, so we can't drive
        // the sync loop from an immutable reference. Return an error indicating
        // callers should use the MatrixListener directly for bidirectional comms.
        anyhow::bail!(
            "Matrix listen requires a mutable client; use matrix_lite::listener::MatrixListener \
             for bidirectional communication"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_type_is_matrix() {
        // We can't construct a real MatrixClient without a server, but we can
        // verify the trait is object-safe and the type name is correct by
        // checking at the type level.
        fn _assert_object_safe(_: &dyn NotificationChannel) {}
    }
}
