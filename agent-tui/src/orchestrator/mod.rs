//! Orchestrator module
//!
//! This module handles multi-agent orchestration, task routing, and execution.

pub mod pool;

pub use pool::{AgentPool, AgentPoolBuilder};

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{
    agent::{AgentHandle, AgentEvent, AgentRuntimeBuilder, AgentInstance},
    llm::LlmClient,
    types::{Agent, Task, TaskResult, TaskStatus, RoutingDecision, RoutingAnalysis, Session, Id, Message, AgentState},
};

/// Dynamic task router
pub struct Router;

impl Router {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a task and determine routing
    pub async fn analyze(&self, task: &Task, session: &Session) -> Result<RoutingAnalysis> {
        // TODO: Implement LLM-based routing analysis
        Ok(RoutingAnalysis {
            task_type: task.task_type,
            suggested_agents: vec![],
            can_parallelize: false,
            estimated_complexity: 5,
            requires_subtasks: false,
        })
    }

    /// Make a routing decision
    pub async fn route(&self, task: Task, analysis: RoutingAnalysis) -> Result<RoutingDecision> {
        // TODO: Implement routing logic
        Ok(RoutingDecision::new(task, vec![], 0.5))
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Task planner for decomposition
pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Self
    }

    /// Decompose a task into subtasks
    pub async fn plan(&self, task: &Task) -> Result<Vec<Task>> {
        // TODO: Implement task decomposition
        Ok(vec![])
    }
}

/// Execution context for a task
pub struct ExecutionContext {
    /// Session ID
    pub session_id: Id,
    /// Message history
    pub messages: Vec<Message>,
    /// Additional context data
    pub context: std::collections::HashMap<String, serde_json::Value>,
}

impl ExecutionContext {
    pub fn new(session_id: Id) -> Self {
        Self {
            session_id,
            messages: vec![],
            context: std::collections::HashMap::new(),
        }
    }

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }
}

/// Task executor that manages agent execution
pub struct Executor {
    /// Agent pool for managing running agents
    pool: AgentPool,
}

impl Executor {
    /// Create a new executor
    pub fn new(
        llm_client: Arc<LlmClient>,
        event_tx: mpsc::Sender<AgentEvent>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            pool: AgentPool::new(max_concurrent, llm_client, event_tx),
        }
    }

    /// Execute a task with a specific agent
    pub async fn execute_task(
        &self,
        agent: Agent,
        task: Task,
        context: ExecutionContext,
    ) -> Result<TaskResult> {
        info!("Executing task {} with agent {}", task.id, agent.name);

        let agent_id = agent.id.clone();
        
        // Get agent from pool or spawn if not running
        let handle = if let Some(handle) = self.pool.get_agent(&agent_id).await {
            handle
        } else {
            self.pool.spawn_agent(agent).await?
        };

        // Execute the task
        let result = handle.process_task(task, context.messages).await;

        result.map_err(|e| anyhow!("Task execution failed: {}", e))
    }

    /// Execute a simple chat request with an agent
    pub async fn execute_chat(
        &self,
        agent: Agent,
        message: String,
        history: Vec<Message>,
    ) -> Result<String> {
        debug!("Executing chat with agent {}", agent.name);

        let agent_id = agent.id.clone();

        // Get agent from pool or spawn if not running
        let handle = if let Some(handle) = self.pool.get_agent(&agent_id).await {
            handle
        } else {
            self.pool.spawn_agent(agent).await?
        };

        // Execute the chat
        let result = handle.chat(message, history).await;

        result.map_err(|e| anyhow!("Chat execution failed: {}", e))
    }

    /// Execute a simple streaming chat request with an agent
    pub async fn execute_chat_streaming(
        &self,
        agent: Agent,
        message: String,
        history: Vec<Message>,
    ) -> Result<String> {
        debug!("Executing streaming chat with agent {}", agent.name);

        let agent_id = agent.id.clone();

        // Get agent from pool or spawn if not running
        let handle = if let Some(handle) = self.pool.get_agent(&agent_id).await {
            handle
        } else {
            self.pool.spawn_agent(agent).await?
        };

        // Execute the streaming chat
        let result = handle.chat_streaming(message, history).await;

        result.map_err(|e| anyhow!("Streaming chat execution failed: {}", e))
    }

    /// Get count of currently active agents
    pub async fn active_count(&self) -> usize {
        self.pool.active_count().await
    }

    /// Check if at capacity
    pub async fn is_at_capacity(&self) -> bool {
        self.pool.is_at_capacity().await
    }

    /// Get agent state
    pub async fn get_agent_state(&self, agent_id: &Id) -> Option<AgentState> {
        self.pool.get_agent_state(agent_id).await
    }

    /// Shutdown all active agents
    pub async fn shutdown_all(&self) -> Result<()> {
        self.pool.shutdown_all().await
    }
}

/// Orchestrator that coordinates routing, planning, and execution
pub struct Orchestrator {
    /// Task router
    router: Router,
    /// Task planner
    planner: Planner,
    /// Task executor
    executor: Executor,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(
        llm_client: Arc<LlmClient>,
        event_tx: mpsc::Sender<AgentEvent>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            router: Router::new(),
            planner: Planner::new(),
            executor: Executor::new(llm_client, event_tx, max_concurrent),
        }
    }

    /// Execute a task with automatic routing
    pub async fn execute_auto(&self, task: Task, session: &Session) -> Result<TaskResult> {
        // Analyze the task
        let analysis = self.router.analyze(&task, session).await?;
        
        // Make routing decision
        let decision = self.router.route(task.clone(), analysis).await?;
        
        // For now, just use the first selected agent or default to a generic response
        // TODO: Actually spawn the selected agents
        if let Some(agent_id) = decision.selected_agents.first() {
            // We would look up the agent and execute with it
            // For now, return a placeholder result
            Ok(TaskResult {
                success: true,
                output: format!("Task routed to agent: {}", agent_id),
                error: None,
                metadata: Default::default(),
            })
        } else {
            Ok(TaskResult {
                success: false,
                output: String::new(),
                error: Some("No agent selected for task".to_string()),
                metadata: Default::default(),
            })
        }
    }

    /// Execute a chat with a specific agent
    pub async fn execute_chat(
        &self,
        agent: Agent,
        message: String,
        history: Vec<Message>,
    ) -> Result<String> {
        self.executor.execute_chat(agent, message, history).await
    }

    /// Execute a streaming chat with a specific agent
    pub async fn execute_chat_streaming(
        &self,
        agent: Agent,
        message: String,
        history: Vec<Message>,
    ) -> Result<String> {
        self.executor.execute_chat_streaming(agent, message, history).await
    }

    /// Execute a task with a specific agent
    pub async fn execute_with_agent(
        &self,
        agent: Agent,
        task: Task,
        context: ExecutionContext,
    ) -> Result<TaskResult> {
        self.executor.execute_task(agent, task, context).await
    }

    /// Get executor reference
    pub fn executor(&self) -> &Executor {
        &self.executor
    }
}
