use std::{collections::HashMap, sync::Arc};

use super::{
    respond_and_set_title::{RespondAndSetTitleTool, SetSessionTitleTool},
    search_memory::MemorySearchTool,
    ToolDefinition,
};
use crate::{
    core::settings::PipelineMode,
    services::llm::CanonicalToolDefinition,
};

/// Contextual filters governing tool availability on a per-turn basis.
#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    pub mode: PipelineMode,
    pub is_first_turn: bool,
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
        reg.register(Arc::new(SetSessionTitleTool));
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
            if !tool.domain().matches(filter.mode) {
                continue;
            }
            if name == "respond_and_set_title" && !filter.title_is_unset {
                log::info!(
                    "[Harness::Tools] '{}' excluded: title already set",
                    name
                );
                continue;
            }
            if name == "search_memory" && !filter.memory_retrieval_enabled {
                log::info!(
                    "[Harness::Tools] '{}' excluded: memory retrieval disabled in settings",
                    name
                );
                continue;
            }
            defs.push(tool.to_canonical(filter.mode));
        }
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// Returns canonical definitions for all registered tools matching the requested pipeline mode.
    pub fn canonical_definitions(&self, mode: PipelineMode) -> Vec<CanonicalToolDefinition> {
        let mut defs: Vec<CanonicalToolDefinition> = self
            .tools
            .values()
            .filter(|t| t.domain().matches(mode))
            .map(|t| t.to_canonical(mode))
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}
