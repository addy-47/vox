//! Generic TCP connectivity health checks for realtime provider WebSocket endpoints.

use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

/// Tests TCP reachability of the given resolved socket address within the specified timeout.
pub(crate) fn tcp_health_check(addr: SocketAddr, timeout: Duration) -> bool {
    std::net::TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// Resolves a `host:port` string to a `SocketAddr`, returning the provided fallback on failure.
pub(crate) fn resolve_or_fallback(host: &str, fallback: SocketAddr) -> SocketAddr {
    host.to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .unwrap_or(fallback)
}
