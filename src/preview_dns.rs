//! Resolve third-party previews once and return only validated public socket addresses.
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, SocketAddr};

pub(crate) struct PublicResolver {
    pub allow_loopback: bool,
}

fn allowed(address: SocketAddr, allow_loopback: bool) -> bool {
    allowed_ip(address.ip(), allow_loopback)
}

/// Apply the same policy to URL literals and resolved socket addresses.
pub(crate) fn allowed_ip(ip: IpAddr, allow_loopback: bool) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    };
    if ip.is_loopback() {
        return allow_loopback;
    }
    if crate::endpoints::is_private_or_loopback_ip(ip) || ip.is_multicast() || ip.is_unspecified() {
        return false;
    }
    match ip {
        IpAddr::V4(v4) => v4.octets()[0] != 0 && v4.octets()[0] < 224,
        IpAddr::V6(v6) => {
            (v6.segments()[0] & 0xe000) == 0x2000
                && !(v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    }
}

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let allow_loopback = self.allow_loopback;
        Box::pin(async move {
            let addresses: Vec<_> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if addresses.is_empty()
                || addresses
                    .iter()
                    .any(|address| !allowed(*address, allow_loopback))
            {
                return Err(std::io::Error::other("Preview DNS target is not public").into());
            }
            // reqwest connects using these exact addresses, without a second DNS lookup.
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_and_mixed_dns_answers_fail_closed() {
        for ip in [
            "127.0.0.1:0",
            "10.0.0.1:0",
            "192.168.1.1:0",
            "[::1]:0",
            "[fe80::1]:0",
            "224.0.0.1:0",
            "[::ffff:127.0.0.1]:0",
            "0.1.2.3:0",
            "[2001:db8::1]:0",
        ] {
            assert!(!allowed(ip.parse().unwrap(), false));
        }
        assert!(allowed("8.8.8.8:0".parse().unwrap(), false));
        assert!(allowed("127.0.0.1:0".parse().unwrap(), true));
    }
}
