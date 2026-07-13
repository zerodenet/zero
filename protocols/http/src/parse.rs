use alloc::string::ToString;
use core::net::{Ipv4Addr, Ipv6Addr};
use core::str::FromStr;

use zero_core::{Address, Error};

pub(crate) fn first_line(request: &[u8]) -> Result<&str, Error> {
    let request = core::str::from_utf8(request)
        .map_err(|_| Error::Protocol("HTTP request is not valid UTF-8"))?;

    request
        .split("\r\n")
        .next()
        .filter(|line| !line.is_empty())
        .ok_or(Error::Protocol("HTTP request line is missing"))
}

pub(crate) fn parse_connect_request(line: &str) -> Result<(Address, u16), Error> {
    let mut parts = line.split_ascii_whitespace();
    let method = parts
        .next()
        .ok_or(Error::Protocol("HTTP method is missing"))?;
    let authority = parts
        .next()
        .ok_or(Error::Protocol("HTTP CONNECT authority is missing"))?;
    let version = parts
        .next()
        .ok_or(Error::Protocol("HTTP version is missing"))?;

    if parts.next().is_some() {
        return Err(Error::Protocol(
            "HTTP request line contains unexpected fields",
        ));
    }

    if method != "CONNECT" {
        return Err(Error::Unsupported("HTTP method is not supported"));
    }

    if !version.starts_with("HTTP/") {
        return Err(Error::Protocol("HTTP version is invalid"));
    }

    parse_authority(authority)
}

fn parse_authority(authority: &str) -> Result<(Address, u16), Error> {
    if authority.is_empty() {
        return Err(Error::Protocol("HTTP CONNECT authority must not be empty"));
    }

    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest
            .find(']')
            .ok_or(Error::Protocol("HTTP CONNECT IPv6 authority is malformed"))?;
        let host = &rest[..end];
        let port = rest[end + 1..]
            .strip_prefix(':')
            .ok_or(Error::Protocol("HTTP CONNECT authority port is missing"))?;

        let addr = Ipv6Addr::from_str(host)
            .map_err(|_| Error::Protocol("HTTP CONNECT IPv6 address is invalid"))?;
        let port = parse_port(port)?;

        return Ok((Address::Ipv6(addr.octets()), port));
    }

    let (host, port) = authority
        .rsplit_once(':')
        .ok_or(Error::Protocol("HTTP CONNECT authority port is missing"))?;
    let port = parse_port(port)?;

    if let Ok(addr) = Ipv4Addr::from_str(host) {
        return Ok((Address::Ipv4(addr.octets()), port));
    }

    if host.contains(':') {
        return Err(Error::Protocol(
            "HTTP CONNECT IPv6 authority must use brackets",
        ));
    }

    if host.is_empty() {
        return Err(Error::Protocol("HTTP CONNECT host must not be empty"));
    }

    Ok((Address::Domain(host.to_string()), port))
}

fn parse_port(port: &str) -> Result<u16, Error> {
    port.parse::<u16>()
        .map_err(|_| Error::Protocol("HTTP CONNECT port is invalid"))
}
