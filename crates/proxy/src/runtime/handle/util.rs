pub(super) fn parse_ip_address(s: &str) -> Option<zero_traits::IpAddress> {
    match s.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => Some(zero_traits::IpAddress::V4(v4.octets())),
        Ok(std::net::IpAddr::V6(v6)) => Some(zero_traits::IpAddress::V6(v6.octets())),
        Err(_) => None,
    }
}
