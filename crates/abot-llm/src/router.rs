use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task types that can be routed to different models
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskType {
    CodeGeneration,
    ArchitectureDesign,
    Debugging,
    GeneralQuery,
    Orchestration,
}

impl TaskType {
    /// Get the default model for this task type
    pub fn default_model(&self) -> &str {
        match self {
            Self::CodeGeneration => "kilo-code",
            Self::ArchitectureDesign => "kilo-architect",
            Self::Debugging => "kilo-debug",
            Self::GeneralQuery => "kilo-ask",
            Self::Orchestration => "kilo-orchestrator",
        }
    }
}

/// Routes tasks to appropriate LLM models based on configuration
pub struct TaskRouter {
    /// Custom model mappings overriding defaults
    model_map: HashMap<TaskType, String>,
}

impl Default for TaskRouter {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}

impl TaskRouter {
    /// Create a new task router with custom model mappings
    pub fn new(model_map: HashMap<TaskType, String>) -> Self {
        Self { model_map }
    }

    /// Route a task type to a model name
    pub fn route(&self, task_type: TaskType) -> String {
        self.model_map
            .get(&task_type)
            .cloned()
            .unwrap_or_else(|| task_type.default_model().to_string())
    }

    /// Set a custom model for a task type
    pub fn set_model(&mut self, task_type: TaskType, model: String) {
        self.model_map.insert(task_type, model);
    }

    /// Get all configured mappings
    pub fn mappings(&self) -> &HashMap<TaskType, String> {
        &self.model_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_defaults() {
        assert_eq!(TaskType::CodeGeneration.default_model(), "kilo-code");
        assert_eq!(
            TaskType::ArchitectureDesign.default_model(),
            "kilo-architect"
        );
        assert_eq!(TaskType::Debugging.default_model(), "kilo-debug");
        assert_eq!(TaskType::GeneralQuery.default_model(), "kilo-ask");
        assert_eq!(TaskType::Orchestration.default_model(), "kilo-orchestrator");
    }

    #[test]
    fn test_router_default_routing() {
        let router = TaskRouter::default();

        assert_eq!(router.route(TaskType::CodeGeneration), "kilo-code");
        assert_eq!(router.route(TaskType::Debugging), "kilo-debug");
    }

    #[test]
    fn test_router_custom_mapping() {
        let mut router = TaskRouter::default();
        router.set_model(TaskType::CodeGeneration, "custom-code-model".to_string());

        assert_eq!(router.route(TaskType::CodeGeneration), "custom-code-model");

        // Other routes should still use defaults
        assert_eq!(router.route(TaskType::Debugging), "kilo-debug");
    }

    #[test]
    fn test_router_multiple_custom_mappings() {
        let mut map = HashMap::new();
        map.insert(TaskType::CodeGeneration, "gpt4-code".to_string());
        map.insert(TaskType::ArchitectureDesign, "gpt4-architect".to_string());

        let router = TaskRouter::new(map);

        assert_eq!(router.route(TaskType::CodeGeneration), "gpt4-code");
        assert_eq!(router.route(TaskType::ArchitectureDesign), "gpt4-architect");
    }
}
