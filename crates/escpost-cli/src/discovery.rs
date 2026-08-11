use std::fmt;
use std::net::Ipv4Addr;

/// The longest network an automatic scan will sweep. A /24 means at most 254
/// probes per interface; anything larger must be requested with --subnet.
#[allow(dead_code)]
pub(crate) const AUTO_SCAN_MINIMUM_PREFIX: u8 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct Subnet {
    network: Ipv4Addr,
    prefix: u8,
}

#[allow(dead_code)]
impl Subnet {
    /// Parse CIDR notation such as `10.42.0.0/24`. Host bits are cleared, so
    /// `10.42.0.71/24` names the same subnet as `10.42.0.0/24`.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let error = || format!("expected CIDR notation such as 10.42.0.0/24, found `{text}`");
        let (address, prefix) = text.split_once('/').ok_or_else(error)?;
        let address = address.trim().parse::<Ipv4Addr>().map_err(|_| error())?;
        let prefix = prefix
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|prefix| *prefix <= 32)
            .ok_or_else(error)?;
        Ok(Self::new(address, prefix))
    }

    fn new(address: Ipv4Addr, prefix: u8) -> Self {
        Self {
            network: Ipv4Addr::from(u32::from(address) & prefix_mask(prefix)),
            prefix,
        }
    }

    /// Derive the connected subnet of an interface address. Returns `None`
    /// for a non-contiguous netmask, which cannot name a CIDR subnet.
    pub(crate) fn from_interface(address: Ipv4Addr, netmask: Ipv4Addr) -> Option<Self> {
        let mask = u32::from(netmask);
        (mask.count_ones() == mask.leading_ones())
            .then(|| Self::new(address, mask.leading_ones() as u8))
    }

    pub(crate) fn prefix(&self) -> u8 {
        self.prefix
    }

    /// Probe candidates. Ordinary subnets exclude the network and broadcast
    /// addresses; /31 and /32 have neither (RFC 3021), so every address is a
    /// host — the integration tests rely on /32 working.
    pub(crate) fn hosts(&self) -> Vec<Ipv4Addr> {
        let network = u32::from(self.network);
        let broadcast = network | !prefix_mask(self.prefix);
        let range = if self.prefix >= 31 {
            network..=broadcast
        } else {
            (network + 1)..=(broadcast - 1)
        };
        range.map(Ipv4Addr::from).collect()
    }
}

impl fmt::Display for Subnet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.network, self.prefix)
    }
}

#[allow(dead_code)]
fn prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::Subnet;

    #[test]
    fn parse_normalizes_host_bits_to_the_network_address() {
        let subnet = Subnet::parse("10.42.0.71/24").expect("a valid CIDR should parse");
        assert_eq!(subnet.to_string(), "10.42.0.0/24");
        assert_eq!(subnet.prefix(), 24);
    }

    #[test]
    fn parse_rejects_text_without_a_prefix() {
        assert!(Subnet::parse("10.42.0.0").is_err());
        assert!(Subnet::parse("10.42.0.0/33").is_err());
        assert!(Subnet::parse("not-an-address/24").is_err());
        assert!(Subnet::parse("10.42.0.0/").is_err());
    }

    #[test]
    fn hosts_exclude_network_and_broadcast_for_ordinary_prefixes() {
        let subnet = Subnet::parse("192.168.7.0/30").expect("a valid CIDR should parse");
        assert_eq!(
            subnet.hosts(),
            vec![Ipv4Addr::new(192, 168, 7, 1), Ipv4Addr::new(192, 168, 7, 2)]
        );
    }

    #[test]
    fn hosts_of_a_24_are_the_254_usable_addresses() {
        let subnet = Subnet::parse("10.42.0.0/24").expect("a valid CIDR should parse");
        let hosts = subnet.hosts();
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts[0], Ipv4Addr::new(10, 42, 0, 1));
        assert_eq!(hosts[253], Ipv4Addr::new(10, 42, 0, 254));
    }

    #[test]
    fn tiny_subnets_probe_every_address() {
        // /31 and /32 have no network or broadcast address (RFC 3021).
        let single = Subnet::parse("127.0.0.1/32").expect("a valid CIDR should parse");
        assert_eq!(single.hosts(), vec![Ipv4Addr::new(127, 0, 0, 1)]);

        let pair = Subnet::parse("10.0.0.0/31").expect("a valid CIDR should parse");
        assert_eq!(
            pair.hosts(),
            vec![Ipv4Addr::new(10, 0, 0, 0), Ipv4Addr::new(10, 0, 0, 1)]
        );
    }

    #[test]
    fn from_interface_derives_the_connected_subnet() {
        let subnet =
            Subnet::from_interface(Ipv4Addr::new(10, 42, 0, 1), Ipv4Addr::new(255, 255, 255, 0))
                .expect("a contiguous netmask should derive a subnet");
        assert_eq!(subnet.to_string(), "10.42.0.0/24");
    }

    #[test]
    fn from_interface_rejects_a_non_contiguous_netmask() {
        assert!(
            Subnet::from_interface(Ipv4Addr::new(10, 42, 0, 1), Ipv4Addr::new(255, 0, 255, 0),)
                .is_none()
        );
    }
}
