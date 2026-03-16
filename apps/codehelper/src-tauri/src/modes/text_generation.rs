use crate::assistant::state::AssistantState;
use async_trait::async_trait;
use smolpc_engine_client::{EngineChatMessage, EngineClient};
use smolpc_engine_core::GenerationConfig;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

const ASSISTANT_CANCELLED: &str = "ASSISTANT_CANCELLED";

#[async_trait]
pub trait TextStreamer: Send + Sync {
    async fn generate_stream(
        &self,
        messages: &[EngineChatMessage],
        state: &AssistantState,
        on_token: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, String>;
}

pub struct EngineTextStreamer {
    client: EngineClient,
}

impl EngineTextStreamer {
    pub fn new(client: EngineClient) -> Self {
        Self { client }
    }

    fn config() -> GenerationConfig {
        GenerationConfig {
            max_length: 1024,
            temperature: 0.55,
            top_k: Some(40),
            top_p: Some(0.9),
            repetition_penalty: 1.08,
            repetition_penalty_last_n: 128,
        }
    }

    fn is_cancelled_error(message: &str) -> bool {
        let normalized = message.to_ascii_uppercase();
        normalized.contains("CANCELLED") || normalized.contains("CANCELED")
    }
}

#[async_trait]
impl TextStreamer for EngineTextStreamer {
    async fn generate_stream(
        &self,
        messages: &[EngineChatMessage],
        state: &AssistantState,
        on_token: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, String> {
        let cancel_client = self.client.clone();
        let reply = Arc::new(Mutex::new(String::new()));
        let reply_for_stream = Arc::clone(&reply);
        let generation = self
            .client
            .generate_stream_messages(messages, Some(Self::config()), |token| {
                if let Ok(mut value) = reply_for_stream.lock() {
                    value.push_str(&token);
                }
                on_token(token);
            });
        tokio::pin!(generation);

        let cancel_future = async {
            loop {
                if state.is_cancelled() {
                    let _ = cancel_client.cancel().await;
                    break;
                }

                sleep(Duration::from_millis(40)).await;
            }
        };
        tokio::pin!(cancel_future);

        let result = tokio::select! {
            result = &mut generation => result,
            _ = &mut cancel_future => generation.await,
        };

        let reply_text = reply.lock().map(|value| value.clone()).unwrap_or_default();

        match result {
            Ok(_) if state.is_cancelled() => Err(ASSISTANT_CANCELLED.to_string()),
            Ok(_) => Ok(reply_text),
            Err(error) => {
                let message = error.to_string();
                if state.is_cancelled() || Self::is_cancelled_error(&message) {
                    Err(ASSISTANT_CANCELLED.to_string())
                } else {
                    Err(format!("Blender generation failed: {message}"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TextStreamer;
    use crate::assistant::state::AssistantState;
    use async_trait::async_trait;
    use smolpc_engine_client::EngineChatMessage;

    struct MockStreamer;

    #[async_trait]
    impl TextStreamer for MockStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            state: &AssistantState,
            on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            on_token("hello ".to_string());
            state.mark_cancelled();
            Err("ASSISTANT_CANCELLED".to_string())
        }
    }

    #[tokio::test]
    async fn text_streamer_trait_can_propagate_cancelled_result() {
        let state = AssistantState::default();
        let mut seen = String::new();

        let error = MockStreamer
            .generate_stream(&[], &state, &mut |token| {
                seen.push_str(&token);
            })
            .await
            .expect_err("stream cancelled");

        assert_eq!(seen, "hello ");
        assert_eq!(error, "ASSISTANT_CANCELLED");
    }
}
