use std::{collections::HashMap, sync::Arc};

use crate::services::llm::CanonicalToolDefinition;

use super::{memory::MemorySearchTool, title::RespondAndSetTitleTool, ToolDefinition};

/// Contextual filters governing tool availability on a per-turn basis.
#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    pub turn_id: u32,
    pub title_is_unset: bool,
    pub memory_retrieval_enabled: bool,
}

/// Registry managing active tool definitions and dynamic per-turn filtering.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolDefinition>>,
}

impl ToolRegistry {
    /// Initializes an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Initializes a tool registry populated with the canonical built-in tools.
    pub fn with_default_tools() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(RespondAndSetTitleTool));
        reg.register(Arc::new(MemorySearchTool));
        reg
    }

    /// Registers a tool definition in the active registry index.
    pub fn register(&mut self, tool: Arc<dyn ToolDefinition>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Retrieves a tool definition by canonical name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolDefinition>> {
        self.tools.get(name).cloned()
    }

    /// Returns canonical definitions for all tools currently eligible under the provided turn filter.
    pub fn active_definitions(&self, filter: &ToolFilter) -> Vec<CanonicalToolDefinition> {
        let mut defs = Vec::new();
        for (name, tool) in &self.tools {
            if name == "respond_and_set_title" && !(filter.turn_id == 1 && filter.title_is_unset) {
                continue;
            }
            if name == "search_memory" && !filter.memory_retrieval_enabled {
                continue;
            }
            defs.push(tool.to_canonical());
        }
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// Returns canonical definitions for all registered tools unconditionally.
    pub fn canonical_definitions(&self) -> Vec<CanonicalToolDefinition> {
        let mut defs: Vec<CanonicalToolDefinition> =
            self.tools.values().map(|t| t.to_canonical()).collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}
