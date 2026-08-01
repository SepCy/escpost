use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::RangeInclusive;

use tokio::net::TcpListener;

/// Why binding a loopback listener failed.
#[derive(Debug)]
pub(crate) enum BindFailure {
    Address {
        address: SocketAddr,
        source: std::io::Error,
    },
    RangeExhausted,
}

/// Bind a loopback TCP listener.
///
/// An explicit address is bound strictly, so a busy port is an error the
/// developer asked for. When no address is given, the first free port in
/// `automatic_ports` is used, so a busy default escalates instead of failing.
pub(crate) async fn bind_loopback(
    requested: Option<SocketAddr>,
    automatic_ports: RangeInclusive<u16>,
) -> Result<TcpListener, BindFailure> {
    if let Some(address) = requested {
        return TcpListener::bind(address)
            .await
            .map_err(|source| BindFailure::Address { address, source });
    }

    for port in automatic_ports {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        match TcpListener::bind(address).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(source) => return Err(BindFailure::Address { address, source }),
        }
    }
    Err(BindFailure::RangeExhausted)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use super::{BindFailure, bind_loopback};

    /// Hold an operating-system-selected loopback port so the tests stay
    /// independent of whatever fixed ports the host happens to use.
    async fn reserved_port() -> (tokio::net::TcpListener, u16) {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("an ephemeral port should be bindable");
        let port = listener
            .local_addr()
            .expect("the listener should have an address")
            .port();
        (listener, port)
    }

    #[tokio::test]
    async fn an_automatic_bind_escalates_past_a_busy_port() {
        let (_occupied, busy) = reserved_port().await;

        let listener = bind_loopback(None, busy..=busy.saturating_add(8))
            .await
            .expect("a free port should follow the busy one");
        let chosen = listener
            .local_addr()
            .expect("the listener should have an address")
            .port();

        assert!(
            chosen > busy,
            "escalation should advance past the busy port"
        );
    }

    #[tokio::test]
    async fn an_explicit_busy_address_is_a_strict_error() {
        let (_occupied, busy) = reserved_port().await;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, busy));

        let outcome = bind_loopback(Some(address), 9000..=9099).await;

        assert!(matches!(outcome, Err(BindFailure::Address { .. })));
    }

    #[tokio::test]
    async fn an_exhausted_range_reports_range_exhausted() {
        let (_occupied, busy) = reserved_port().await;

        // The only port in the range is already taken, so there is nowhere to go.
        let outcome = bind_loopback(None, busy..=busy).await;

        assert!(matches!(outcome, Err(BindFailure::RangeExhausted)));
    }
}
