//! The headless reference client.
//!
//! **Owns:** the headless reference client used by the conformance suite and
//! by the end-to-end remote turn suite.
//!
//! **Must never contain:** production logic. Every behaviour this client
//! exercises must exist in a production module first; the client may not
//! become a second implementation of the protocol.
//!
//! Workspace rule: `AGENTS.md` §7.1, `testing/`.
