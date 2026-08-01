use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationEdge {
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BatchEvaluationResult {
    pub item_id: i64,
    pub is_superseded: bool,
    pub superseded_by: Option<String>,
    pub relations: Vec<RelationEdge>,
}
