use query_sieve::MemoryScope;
use crate::core::constants::MemoryCollection;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chitchat_scope_prunes_all_collections() {
        let routing = route_scope(MemoryScope::ChitChat);
        assert!(routing.sql_collections.is_empty());
        assert!(routing.vector_collections.is_empty());
    }

    #[test]
    fn test_user_scope_routing() {
        let routing = route_scope(MemoryScope::User);
        assert!(routing.sql_collections.is_empty());
        assert_eq!(routing.vector_collections, vec![MemoryCollection::Profile, MemoryCollection::Constraints]);
    }

    #[test]
    fn test_domain_scope_routing() {
        let routing = route_scope(MemoryScope::Domain);
        assert!(routing.sql_collections.is_empty());
        assert_eq!(
            routing.vector_collections,
            vec![MemoryCollection::Entities, MemoryCollection::Directives, MemoryCollection::Constraints]
        );
    }

    #[test]
    fn test_temporal_scope_routing() {
        let routing = route_scope(MemoryScope::Temporal);
        assert_eq!(routing.sql_collections, vec![MemoryCollection::Directives, MemoryCollection::Narrative]);
        assert_eq!(routing.vector_collections, vec![MemoryCollection::Constraints]);
    }
}
