//! Shared state module
//!
//! This module provides shared memory and state management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Shared memory with hierarchical namespaces
pub struct SharedMemory {
    /// Global scope (shared across all sessions)
    global: Arc<RwLock<HashMap<String, Value>>>,
    /// Session scope (shared within a session)
    sessions: Arc<RwLock<HashMap<String, HashMap<String, Value>>>>,
    /// Agent scope (per-agent memory)
    agents: Arc<RwLock<HashMap<String, HashMap<String, Value>>>>,
}

impl SharedMemory {
    /// Create a new shared memory instance
    pub fn new() -> Self {
        Self {
            global: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a value from global scope
    pub fn get_global(&self, key: &str) -> Option<Value> {
        self.global.read().ok()?.get(key).cloned()
    }

    /// Set a value in global scope
    pub fn set_global(&self, key: &str, value: Value) {
        if let Ok(mut guard) = self.global.write() {
            guard.insert(key.to_string(), value);
        }
    }

    /// Get a value from session scope
    pub fn get_session(&self, session_id: &str, key: &str) -> Option<Value> {
        self.sessions
            .read()
            .ok()?
            .get(session_id)?
            .get(key)
            .cloned()
    }

    /// Set a value in session scope
    pub fn set_session(&self, session_id: &str, key: &str, value: Value) {
        if let Ok(mut guard) = self.sessions.write() {
            guard
                .entry(session_id.to_string())
                .or_insert_with(HashMap::new)
                .insert(key.to_string(), value);
        }
    }

    /// Get a value from agent scope
    pub fn get_agent(&self, agent_id: &str, key: &str) -> Option<Value> {
        self.agents.read().ok()?.get(agent_id)?.get(key).cloned()
    }

    /// Set a value in agent scope
    pub fn set_agent(&self, agent_id: &str, key: &str, value: Value) {
        if let Ok(mut guard) = self.agents.write() {
            guard
                .entry(agent_id.to_string())
                .or_insert_with(HashMap::new)
                .insert(key.to_string(), value);
        }
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution context for tasks
pub struct ExecutionContext {
    /// Session ID
    pub session_id: String,
    /// Parent task ID (if any)
    pub parent_task: Option<String>,
    /// Execution depth
    pub depth: u32,
    /// Maximum depth
    pub max_depth: u32,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            parent_task: None,
            depth: 0,
            max_depth: 5,
        }
    }

    /// Create a child context
    pub fn child(&self, task_id: &str) -> Self {
        Self {
            session_id: self.session_id.clone(),
            parent_task: Some(task_id.to_string()),
            depth: self.depth + 1,
            max_depth: self.max_depth,
        }
    }
}
