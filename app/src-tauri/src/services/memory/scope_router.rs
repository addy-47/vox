use crate::core::constants::MemoryCollection;
use query_sieve::MemoryScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeRouting {
    pub sql_collections: Vec<MemoryCollection>,
    pub vector_collections: Vec<MemoryCollection>,
}

/// Computes the v7 collection pruning matrix for a given `MemoryScope` (spec §5).
pub fn route_scope(scope: MemoryScope) -> ScopeRouting {
    match scope {
        MemoryScope::ChitChat => ScopeRouting {
            sql_collections: Vec::new(),
            vector_collections: Vec::new(),
        },
        MemoryScope::User => ScopeRouting {
            sql_collections: Vec::new(),
            vector_collections: vec![MemoryCollection::Profile, MemoryCollection::Constraints],
        },
        MemoryScope::Domain => ScopeRouting {
            sql_collections: Vec::new(),
            vector_collections: vec![
                MemoryCollection::Entities,
                MemoryCollection::Directives,
                MemoryCollection::Constraints,
            ],
        },
        MemoryScope::Temporal => ScopeRouting {
            sql_collections: vec![MemoryCollection::Directives, MemoryCollection::Narrative],
            vector_collections: vec![MemoryCollection::Constraints],
        },
    }
}
