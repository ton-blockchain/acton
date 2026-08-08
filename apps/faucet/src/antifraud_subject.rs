use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub(crate) fn wallet(address: &str) -> String {
    format!("wallet:{address}")
}

pub(crate) fn github(github_user_id: u64) -> String {
    format!("github:{github_user_id}")
}

pub(crate) fn client_ip(ip: IpAddr) -> String {
    let ip = match ip {
        IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some() => {
            IpAddr::V4(ip.to_ipv4_mapped().expect("checked IPv4-mapped address"))
        }
        IpAddr::V6(ip) => {
            let network = u128::from(ip) & (u128::MAX << 64);
            IpAddr::V6(Ipv6Addr::from(network))
        }
        IpAddr::V4(ip) => IpAddr::V4(ip),
    };
    format!("client-ip:{ip}")
}

pub(crate) fn client_subnet(ip: IpAddr, ipv4_prefix_length: u32) -> String {
    let subnet = match ip {
        IpAddr::V4(ip) => ipv4_subnet(ip, ipv4_prefix_length),
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(ip) => ipv4_subnet(ip, ipv4_prefix_length),
            None => ipv6_subnet(ip),
        },
    };

    format!("client-subnet:{subnet}")
}

pub(crate) fn device_uid(device_uid: &str) -> String {
    format!("device-uid:{}", device_uid.to_ascii_lowercase())
}

fn ipv4_subnet(ip: Ipv4Addr, prefix_length: u32) -> String {
    debug_assert!(prefix_length <= 32);
    let host_bits = 32 - prefix_length;
    let mask = u32::MAX.checked_shl(host_bits).unwrap_or(0);
    let network = Ipv4Addr::from(u32::from(ip) & mask);
    format!("{network}/{prefix_length}")
}

fn ipv6_subnet(ip: Ipv6Addr) -> String {
    let network = Ipv6Addr::from(u128::from(ip) & (u128::MAX << 64));
    format!("{network}/64")
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::{client_ip, client_subnet, device_uid, wallet};

    #[test]
    fn builds_wallet_subject() {
        assert_eq!(wallet("0:abc"), "wallet:0:abc");
    }

    #[test]
    fn builds_client_subjects_from_peer_ip() {
        assert_eq!(
            client_ip(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))),
            "client-ip:203.0.113.7"
        );
        assert_eq!(
            client_ip(IpAddr::V6(
                "2001:db8:1234:5678:abcd::1".parse::<Ipv6Addr>().unwrap()
            )),
            "client-ip:2001:db8:1234:5678::"
        );
        assert_eq!(
            client_ip(IpAddr::V6("::ffff:192.0.2.44".parse::<Ipv6Addr>().unwrap())),
            "client-ip:192.0.2.44"
        );
    }

    #[test]
    fn builds_ipv4_subnet_subjects_for_boundary_prefixes() {
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));

        assert_eq!(client_subnet(ip, 0), "client-subnet:0.0.0.0/0");
        assert_eq!(client_subnet(ip, 24), "client-subnet:203.0.113.0/24");
        assert_eq!(client_subnet(ip, 32), "client-subnet:203.0.113.7/32");
    }

    #[test]
    fn builds_subnet_subjects_for_ipv4_mapped_and_native_ipv6() {
        assert_eq!(
            client_subnet(
                IpAddr::V6("::ffff:192.0.2.44".parse::<Ipv6Addr>().unwrap()),
                24
            ),
            "client-subnet:192.0.2.0/24"
        );
        assert_eq!(
            client_subnet(
                IpAddr::V6("2001:db8:1234:5678:abcd::1".parse::<Ipv6Addr>().unwrap()),
                24
            ),
            "client-subnet:2001:db8:1234:5678::/64"
        );
    }

    #[test]
    fn builds_case_normalized_device_subject() {
        assert_eq!(
            device_uid("550E8400-E29B-41D4-A716-446655440000"),
            "device-uid:550e8400-e29b-41d4-a716-446655440000"
        );
    }
}
