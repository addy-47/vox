use query_sieve::MemoryScope;

use crate::services::memory::MemoryCollection;

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

    /// Tests that ChitChat scope prunes both SQL and vector retrieval completely.
    #[test]
    fn test_route_scope_chitchat() {
        let routing = route_scope(MemoryScope::ChitChat);
        assert!(routing.sql_collections.is_empty());
        assert!(routing.vector_collections.is_empty());
    }

    /// Tests that User scope routes exclusively to Profile and Constraints vector collections.
    #[test]
    fn test_route_scope_user() {
        let routing = route_scope(MemoryScope::User);
        assert!(routing.sql_collections.is_empty());
        assert_eq!(
            routing.vector_collections,
            vec![MemoryCollection::Profile, MemoryCollection::Constraints]
        );
    }

    /// Tests that Domain scope routes exclusively to Entities, Directives, and Constraints vector collections.
    #[test]
    fn test_route_scope_domain() {
        let routing = route_scope(MemoryScope::Domain);
        assert!(routing.sql_collections.is_empty());
        assert_eq!(
            routing.vector_collections,
            vec![
                MemoryCollection::Entities,
                MemoryCollection::Directives,
                MemoryCollection::Constraints,
            ]
        );
    }

    /// Tests that Temporal scope routes Directives and Narrative to SQL and Constraints to vector.
    #[test]
    fn test_route_scope_temporal() {
        let routing = route_scope(MemoryScope::Temporal);
        assert_eq!(
            routing.sql_collections,
            vec![MemoryCollection::Directives, MemoryCollection::Narrative]
        );
        assert_eq!(
            routing.vector_collections,
            vec![MemoryCollection::Constraints]
        );
    }

    /// Tests invariant that Identity collection is never routed to SQL across any scope.
    #[test]
    fn test_route_scope_identity_exclusion() {
        let scopes = [
            MemoryScope::ChitChat,
            MemoryScope::User,
            MemoryScope::Domain,
            MemoryScope::Temporal,
        ];
        for scope in scopes {
            let routing = route_scope(scope);
            assert!(
                !routing
                    .sql_collections
                    .contains(&MemoryCollection::Identity),
                "Identity collection must never be queried via SQL in scope {:?}",
                scope
            );
        }
    }
}
