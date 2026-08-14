use std::future::Future;
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
    bind_loopback_with(requested, automatic_ports, |address| {
        TcpListener::bind(address)
    })
    .await
}

async fn bind_loopback_with<T, F, Fut>(
    requested: Option<SocketAddr>,
    automatic_ports: RangeInclusive<u16>,
    mut bind: F,
) -> Result<T, BindFailure>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, std::io::Error>>,
{
    if let Some(address) = requested {
        return bind(address)
            .await
            .map_err(|source| BindFailure::Address { address, source });
    }

    for port in automatic_ports {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        match bind(address).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(source) => return Err(BindFailure::Address { address, source }),
        }
    }
    Err(BindFailure::RangeExhausted)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::{Error, ErrorKind};
    use std::net::SocketAddr;

    use super::{BindFailure, bind_loopback_with};

    #[tokio::test]
    async fn an_automatic_bind_escalates_past_a_busy_port() {
        let mut outcomes =
            VecDeque::from([Err(Error::from(ErrorKind::AddrInUse)), Ok("second port")]);
        let mut attempts = Vec::new();
        let listener = bind_loopback_with(None, 9000..=9002, |address| {
            attempts.push(address);
            std::future::ready(outcomes.pop_front().expect("one outcome per attempt"))
        })
        .await
        .expect("a free port should follow the busy one");

        assert_eq!(listener, "second port");
        assert_eq!(
            attempts,
            [
                "127.0.0.1:9000"
                    .parse::<SocketAddr>()
                    .expect("valid address"),
                "127.0.0.1:9001"
                    .parse::<SocketAddr>()
                    .expect("valid address"),
            ]
        );
    }

    #[tokio::test]
    async fn an_explicit_busy_address_is_a_strict_error() {
        let address: SocketAddr = "127.0.0.1:9000".parse().expect("valid address");

        let mut attempts = Vec::new();
        let outcome = bind_loopback_with(Some(address), 9000..=9099, |attempt| {
            attempts.push(attempt);
            std::future::ready(Err::<(), _>(Error::from(ErrorKind::AddrInUse)))
        })
        .await;

        assert!(matches!(outcome, Err(BindFailure::Address { .. })));
        assert_eq!(attempts, [address]);
    }

    #[tokio::test]
    async fn an_exhausted_range_reports_range_exhausted() {
        let mut attempts = Vec::new();
        let outcome = bind_loopback_with(None, 9000..=9002, |address| {
            attempts.push(address);
            std::future::ready(Err::<(), _>(Error::from(ErrorKind::AddrInUse)))
        })
        .await;

        assert!(matches!(outcome, Err(BindFailure::RangeExhausted)));
        assert_eq!(
            attempts,
            (9000..=9002)
                .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
                .collect::<Vec<_>>()
        );
    }
}
