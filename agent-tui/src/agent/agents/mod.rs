pub mod coder;
pub mod explorer;
pub mod integrator;
pub mod planner;
pub mod reviewer;
pub mod tester;

#[allow(unused_imports)]
pub use coder::{CodeBlock, CodeChange, CoderAgent, FileOperation};
pub use explorer::ExplorerAgent;
pub use integrator::IntegratorAgent;
pub use planner::PlannerAgent;
pub use reviewer::ReviewerAgent;
pub use tester::TesterAgent;

use crate::types::{Agent, Task, TaskResult};
use crate::agent::TaskProcessor;
use crate::shared::SharedMemory;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use regex::Regex;

/// Review comment from ReviewerAgent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewComment {
    pub file_path: Option<String>,
    pub line_number: Option<u32>,
    pub severity: ReviewSeverity,
    pub message: String,
    pub snippet: Option<String>,
}

/// Severity level of a review comment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewSeverity {
    Critical,
    Major,
    Minor,
    Style,
    Security,
}

impl ReviewSeverity {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" | "blocker" | "error" => ReviewSeverity::Critical,
            "major" | "warning" => ReviewSeverity::Major,
            "minor" | "info" => ReviewSeverity::Minor,
            "style" | "lint" => ReviewSeverity::Style,
            "security" | "vulnerability" => ReviewSeverity::Security,
            _ => ReviewSeverity::Minor,
        }
    }
}

/// Review result with scores and comments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewResult {
    pub quality_score: u8, // 0-100
    pub security_score: u8, // 0-100
    pub maintainability_score: u8, // 0-100
    pub comments: Vec<ReviewComment>,
    pub summary: String,
}

/// Planner agent for task decomposition
#[allow(dead_code)]
pub struct PlannerAgent;

impl TaskProcessor for PlannerAgent {
    fn process_task(&self, _task: &Task, response: &str, _shared_memory: Arc<SharedMemory>) -> Result<TaskResult> {
        let mut metadata = HashMap::new();
        
        // Try to extract JSON plan from response
        let json_re = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```").context("Failed to compile regex")?;
        if let Some(cap) = json_re.captures(response) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(cap.get(1).unwrap().as_str()) {
                metadata.insert("plan".to_string(), json_val);
                metadata.insert("has_structured_plan".to_string(), serde_json::json!(true));
            }
        }
        
        Ok(TaskResult {
            success: true,
            output: response.to_string(),
            error: None,
            metadata,
        })
    }
}

#[allow(dead_code)]
impl PlannerAgent {
    pub fn create() -> Agent {
        Agent::new("planner", crate::types::AgentRole::Planner, "gpt-4o")
            .with_description("Plans and orchestrates multi-agent workflows")
            .with_capabilities(vec![crate::types::Capability::Plan])
            .with_system_prompt(
                "You are a task planner and orchestrator. Your job is to:\n\
                1. Analyze user requests and understand the goal\n\
                2. Break down complex tasks into manageable subtasks\n\
                3. Determine which specialized agents are needed\n\
                4. Create a clear execution plan\n\
                5. Provide reasoning for your decisions\n\n\
                Be concise but thorough in your planning. Consider dependencies between tasks.\n\n\
                ALWAYS provide your plan in a JSON code block using this format:\n\
                ```json\n\
                {\n  \"subtasks\": [\n    {\"description\": \"...\", \"agent\": \"...\", \"dependencies\": []}\n  ]\n}\n\
                ```"
            )
    }
}

/// Reviewer agent for code review
#[allow(dead_code)]
pub struct ReviewerAgent;

impl TaskProcessor for ReviewerAgent {
    fn process_task(&self, _task: &Task, response: &str, _shared_memory: Arc<SharedMemory>) -> Result<TaskResult> {
        let result = Self::parse_review_response(response)?;
        
        let mut metadata = HashMap::new();
        metadata.insert("quality_score".to_string(), serde_json::json!(result.quality_score));
        metadata.insert("security_score".to_string(), serde_json::json!(result.security_score));
        metadata.insert("maintainability_score".to_string(), serde_json::json!(result.maintainability_score));
        metadata.insert("comment_count".to_string(), serde_json::json!(result.comments.len()));
        metadata.insert("comments".to_string(), serde_json::json!(result.comments));
        
        Ok(TaskResult {
            success: true,
            output: response.to_string(),
            error: None,
            metadata,
        })
    }
}

#[allow(dead_code)]
impl ReviewerAgent {
    pub fn create() -> Agent {
        Agent::new("reviewer", crate::types::AgentRole::Reviewer, "gpt-4o-mini")
            .with_description("Reviews code for quality and issues")
            .with_capabilities(vec![crate::types::Capability::Review])
            .with_system_prompt(
                "You are a code reviewer. Your job is to:\n\
                1. Identify bugs and potential issues\n\
                2. Check for security vulnerabilities\n\
                3. Ensure code follows best practices\n\
                4. Verify error handling is appropriate\n\
                5. Suggest improvements for clarity and performance\n\n\
                Be constructive in your feedback. Prioritize critical issues over style preferences.\n\n\
                At the end of your review, ALWAYS provide a scores section in this format:\n\
                ---\n\
                Scores:\n\
                Quality: 0-100\n\
                Security: 0-100\n\
                Maintainability: 0-100\n\
                ---"
            )
    }

    /// Parse the LLM response to extract scores and structured comments
    pub fn parse_review_response(response: &str) -> Result<ReviewResult> {
        let mut quality_score = 70;
        let mut security_score = 70;
        let mut maintainability_score = 70;
        let mut comments = Vec::new();
        
        // Extract scores using regex
        let quality_re = Regex::new(r"Quality:\s*(\d+)").context("Failed to compile regex")?;
        let security_re = Regex::new(r"Security:\s*(\d+)").context("Failed to compile regex")?;
        let maintain_re = Regex::new(r"Maintainability:\s*(\d+)").context("Failed to compile regex")?;
        
        if let Some(cap) = quality_re.captures(response) {
            quality_score = cap.get(1).unwrap().as_str().parse().unwrap_or(70);
        }
        if let Some(cap) = security_re.captures(response) {
            security_score = cap.get(1).unwrap().as_str().parse().unwrap_or(70);
        }
        if let Some(cap) = maintain_re.captures(response) {
            maintainability_score = cap.get(1).unwrap().as_str().parse().unwrap_or(70);
        }

        // Simple comment extraction (looking for lists with severity)
        let comment_re = Regex::new(r"(?m)^\s*[\-\*]\s*\[(Critical|Major|Minor|Style|Security)\]\s*(?:(?:in\s+)?([^:\n]+)(?::(\d+))?:\s*)?(.+)$").context("Failed to compile regex")?;
        
        for cap in comment_re.captures_iter(response) {
            let severity = ReviewSeverity::from_str(cap.get(1).unwrap().as_str());
            let file_path = cap.get(2).map(|m| m.as_str().trim().to_string());
            let line_number = cap.get(3).and_then(|m| m.as_str().parse().ok());
            let message = cap.get(4).unwrap().as_str().trim().to_string();
            
            comments.push(ReviewComment {
                file_path,
                line_number,
                severity,
                message,
                snippet: None,
            });
        }

        Ok(ReviewResult {
            quality_score,
            security_score,
            maintainability_score,
            comments,
            summary: response.to_string(),
        })
    }
}

/// Tester agent for test generation
#[allow(dead_code)]
pub struct TesterAgent;

impl TaskProcessor for TesterAgent {
    fn process_task(&self, _task: &Task, response: &str, _shared_memory: Arc<SharedMemory>) -> Result<TaskResult> {
        let mut metadata = HashMap::new();
        
        // Extract test cases and results
        let test_case_re = Regex::new(r"(?m)^\s*[\-\*]\s*\[(PASS|FAIL|SKIP)\]\s*(.+)$").context("Failed to compile regex")?;
        let mut test_cases = Vec::new();
        let mut passed = 0;
        let mut failed = 0;
        
        for cap in test_case_re.captures_iter(response) {
            let status = cap.get(1).unwrap().as_str();
            let name = cap.get(2).unwrap().as_str().trim().to_string();
            
            match status {
                "PASS" => passed += 1,
                "FAIL" => failed += 1,
                _ => {}
            }
            
            test_cases.push(serde_json::json!({
                "name": name,
                "status": status,
            }));
        }
        
        metadata.insert("test_cases".to_string(), serde_json::json!(test_cases));
        metadata.insert("passed_count".to_string(), serde_json::json!(passed));
        metadata.insert("failed_count".to_string(), serde_json::json!(failed));
        
        // Extract code blocks (the generated tests)
        let blocks = crate::agent::agents::coder::CoderAgent::extract_code_blocks(response).unwrap_or_default();
        metadata.insert("generated_tests_count".to_string(), serde_json::json!(blocks.len()));
        
        Ok(TaskResult {
            success: failed == 0,
            output: response.to_string(),
            error: if failed > 0 { Some(format!("{} tests failed", failed)) } else { None },
            metadata,
        })
    }
}

#[allow(dead_code)]
impl TesterAgent {
    pub fn create() -> Agent {
        Agent::new("tester", crate::types::AgentRole::Tester, "gpt-4o-mini")
            .with_description("Generates and runs tests")
            .with_capabilities(vec![crate::types::Capability::Test])
            .with_system_prompt(
                "You are a testing specialist. Your job is to:\n\
                1. Write comprehensive unit tests\n\
                2. Create integration tests where appropriate\n\
                3. Test edge cases and error conditions\n\
                4. Use appropriate testing frameworks\n\
                5. Ensure good test coverage\n\n\
                Write tests that are clear, maintainable, and validate the expected behavior.\n\n\
                When reporting test results, use this format:\n\
                * [PASS] test_name\n\
                * [FAIL] test_name\n"
            )
    }
}

/// Explorer agent for codebase exploration
#[allow(dead_code)]
pub struct ExplorerAgent;

impl TaskProcessor for ExplorerAgent {
    fn process_task(&self, _task: &Task, response: &str, _shared_memory: Arc<SharedMemory>) -> Result<TaskResult> {
        let mut metadata = HashMap::new();
        
        // Extract found files
        let file_re = Regex::new(r"(?m)^\s*[\-\*]\s*File:\s*([^\n\s]+)").context("Failed to compile regex")?;
        let mut files = Vec::new();
        for cap in file_re.captures_iter(response) {
            files.push(cap.get(1).unwrap().as_str().to_string());
        }
        
        // Extract found symbols (functions, classes, etc.)
        let symbol_re = Regex::new(r"(?m)^\s*[\-\*]\s*Symbol:\s*([^\n\s]+)").context("Failed to compile regex")?;
        let mut symbols = Vec::new();
        for cap in symbol_re.captures_iter(response) {
            symbols.push(cap.get(1).unwrap().as_str().to_string());
        }
        
        metadata.insert("discovered_files".to_string(), serde_json::json!(files));
        metadata.insert("discovered_symbols".to_string(), serde_json::json!(symbols));
        
        Ok(TaskResult {
            success: true,
            output: response.to_string(),
            error: None,
            metadata,
        })
    }
}

#[allow(dead_code)]
impl ExplorerAgent {
    pub fn create() -> Agent {
        Agent::new("explorer", crate::types::AgentRole::Explorer, "gpt-4o-mini")
            .with_description("Explores codebase structure and files")
            .with_capabilities(vec![crate::types::Capability::Explore])
            .with_system_prompt(
                "You are a codebase explorer. Your job is to:\n\
                1. Navigate and understand code structure\n\
                2. Find relevant files and functions\n\
                3. Analyze dependencies and relationships\n\
                4. Gather context for other agents\n\
                5. Summarize findings clearly\n\n\
                Be thorough in your exploration and provide detailed context about what you find.\n\n\
                When listing discovered items, use this format:\n\
                * File: path/to/file\n\
                * Symbol: function_or_class_name"
            )
    }
}

/// Integrator agent for result synthesis
#[allow(dead_code)]
pub struct IntegratorAgent;

impl TaskProcessor for IntegratorAgent {
    fn process_task(&self, _task: &Task, response: &str, _shared_memory: Arc<SharedMemory>) -> Result<TaskResult> {
        Ok(TaskResult {
            success: true,
            output: response.to_string(),
            error: None,
            metadata: Default::default(),
        })
    }
}

#[allow(dead_code)]
impl IntegratorAgent {
    pub fn create() -> Agent {
        Agent::new("integrator", crate::types::AgentRole::Integrator, "gpt-4o")
            .with_description("Synthesizes results from multiple agents")
            .with_capabilities(vec![crate::types::Capability::Document])
            .with_system_prompt(
                "You are a results integrator. Your job is to:\n\
                1. Synthesize outputs from multiple agents\n\
                2. Resolve conflicts between different approaches\n\
                3. Create cohesive final deliverables\n\
                4. Ensure consistency across all components\n\
                5. Provide clear summaries and documentation\n\n\
                Create well-structured, comprehensive outputs that combine the best from all sources."
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskType;

    #[test]
    fn test_reviewer_parse_scores() {
        let response = "Review complete.\n\nScores:\nQuality: 85\nSecurity: 90\nMaintainability: 80\n---";
        let result = ReviewerAgent::parse_review_response(response).unwrap();
        assert_eq!(result.quality_score, 85);
        assert_eq!(result.security_score, 90);
        assert_eq!(result.maintainability_score, 80);
    }

    #[test]
    fn test_reviewer_parse_comments() {
        let response = "Issues found:\n* [Critical] in src/main.rs:10: Null pointer potential\n* [Style] formatting is off";
        let result = ReviewerAgent::parse_review_response(response).unwrap();
        assert_eq!(result.comments.len(), 2);
        assert_eq!(result.comments[0].severity, ReviewSeverity::Critical);
        assert_eq!(result.comments[0].file_path, Some("src/main.rs".to_string()));
        assert_eq!(result.comments[0].line_number, Some(10));
        assert_eq!(result.comments[1].severity, ReviewSeverity::Style);
    }

    #[test]
    fn test_planner_extract_plan() {
        let response = "Plan:\n```json\n{\"subtasks\": [{\"description\": \"test\", \"agent\": \"coder\"}]}\n```";
        let agent = PlannerAgent;
        let task = Task::new("desc", TaskType::Planning);
        let shared_memory = Arc::new(SharedMemory::new());
        let result = agent.process_task(&task, response, shared_memory).unwrap();
        assert!(result.metadata.contains_key("plan"));
        assert_eq!(result.metadata["has_structured_plan"], serde_json::json!(true));
    }

    #[test]
    fn test_tester_extract_results() {
        let response = "Results:\n* [PASS] test_1\n* [FAIL] test_2\n```rust\nfn test() {}\n```";
        let agent = TesterAgent;
        let task = Task::new("desc", TaskType::TestExecution);
        let shared_memory = Arc::new(SharedMemory::new());
        let result = agent.process_task(&task, response, shared_memory).unwrap();
        assert_eq!(result.metadata["passed_count"], serde_json::json!(1));
        assert_eq!(result.metadata["failed_count"], serde_json::json!(1));
        assert_eq!(result.metadata["generated_tests_count"], serde_json::json!(1));
        assert!(!result.success);
    }

    #[test]
    fn test_explorer_extract_info() {
        let response = "Found:\n* File: src/lib.rs\n* Symbol: my_func";
        let agent = ExplorerAgent;
        let task = Task::new("desc", TaskType::Exploration);
        let shared_memory = Arc::new(SharedMemory::new());
        let result = agent.process_task(&task, response, shared_memory).unwrap();
        assert_eq!(result.metadata["discovered_files"], serde_json::json!(vec!["src/lib.rs"]));
        assert_eq!(result.metadata["discovered_symbols"], serde_json::json!(vec!["my_func"]));
    }
}

/// Initialize all built-in agents
pub fn initialize_default_agents(registry: &mut crate::agent::AgentRegistry) {
    registry.register(PlannerAgent::create());
    registry.register(CoderAgent::create());
    registry.register(ReviewerAgent::create());
    registry.register(TesterAgent::create());
    registry.register(ExplorerAgent::create());
    registry.register(IntegratorAgent::create());
}
