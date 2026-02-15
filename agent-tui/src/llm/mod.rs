//! LLM integration module
//! 
//! This module handles communication with LLM providers (currently OpenAI).

use anyhow::Result;
use crate::types::Message;

/// LLM client for making API calls
pub struct LlmClient {
    api_key: String,
    model: String,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Send a message and get a response
    pub async fn send_message(&self, messages: &[Message]) -> Result<String> {
        // TODO: Implement OpenAI API integration
        Ok("LLM integration not yet implemented".to_string())
    }

    /// Send a streaming message
    pub async fn send_message_streaming(
        &self,
        messages: &[Message],
    ) -> Result<impl futures::Stream<Item = Result<String>>> {
        // TODO: Implement streaming
        use futures::stream;
        Ok(stream::once(async { Ok("Streaming not yet implemented".to_string()) }))
    }
}
