use crate::types::Agent;

/// Built-in agent implementations

/// Planner agent for task decomposition
pub struct PlannerAgent;

impl PlannerAgent {
    pub fn create() -> Agent {
        Agent::new("planner", crate::types::AgentRole::Planner, "gpt-4o")
            .with_description("Plans and orchestrates multi-agent workflows")
            .with_capabilities(vec![crate::types::Capability::Plan])
    }
}

/// Coder agent for code generation
pub struct CoderAgent;

impl CoderAgent {
    pub fn create() -> Agent {
        Agent::new("coder", crate::types::AgentRole::Coder, "gpt-4o")
            .with_description("Writes and modifies code")
            .with_capabilities(vec![
                crate::types::Capability::Code,
                crate::types::Capability::Refactor,
                crate::types::Capability::Debug,
            ])
    }
}

/// Reviewer agent for code review
pub struct ReviewerAgent;

impl ReviewerAgent {
    pub fn create() -> Agent {
        Agent::new("reviewer", crate::types::AgentRole::Reviewer, "gpt-4o-mini")
            .with_description("Reviews code for quality and issues")
            .with_capabilities(vec![crate::types::Capability::Review])
    }
}

/// Tester agent for test generation
pub struct TesterAgent;

impl TesterAgent {
    pub fn create() -> Agent {
        Agent::new("tester", crate::types::AgentRole::Tester, "gpt-4o-mini")
            .with_description("Generates and runs tests")
            .with_capabilities(vec![crate::types::Capability::Test])
    }
}

/// Explorer agent for codebase exploration
pub struct ExplorerAgent;

impl ExplorerAgent {
    pub fn create() -> Agent {
        Agent::new("explorer", crate::types::AgentRole::Explorer, "gpt-4o-mini")
            .with_description("Explores codebase structure and files")
            .with_capabilities(vec![crate::types::Capability::Explore])
    }
}

/// Integrator agent for result synthesis
pub struct IntegratorAgent;

impl IntegratorAgent {
    pub fn create() -> Agent {
        Agent::new("integrator", crate::types::AgentRole::Integrator, "gpt-4o")
            .with_description("Synthesizes results from multiple agents")
            .with_capabilities(vec![crate::types::Capability::Document])
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
