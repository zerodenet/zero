//! Trojan protocol constants and helpers.

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

/// Trojan protocol request command types.
pub const CMD_TCP: u8 = 0x01;
pub const CMD_UDP: u8 = 0x03;

/// Address type constants (socks5 compatible).
pub const ATYP_IPV4: u8 = 0x01;
pub const ATYP_DOMAIN: u8 = 0x03;
pub const ATYP_IPV6: u8 = 0x04;

/// CRLF delimiter used in Trojan protocol.
pub const CRLF: &[u8] = b"\r\n";

/// Length of SHA224 hex password in bytes.
pub const PASSWORD_HASH_LEN: usize = 56;

/// Read the password hash + CRLF from a stream.
///
/// Returns the hex password string on success.
pub async fn read_password<S: AsyncSocket>(stream: &mut S) -> Result<String, Error> {
    let mut buf = [0u8; PASSWORD_HASH_LEN + 2]; // hash + CRLF
    read_exact(stream, &mut buf).await?;
    if &buf[PASSWORD_HASH_LEN..] != CRLF {
        return Err(Error::Protocol("trojan: expected CRLF after password hash"));
    }
    let hex = std::str::from_utf8(&buf[..PASSWORD_HASH_LEN])
        .map_err(|_| Error::Protocol("trojan: invalid password hash"))?;
    Ok(hex.to_owned())
}

/// Write password hash + CRLF to a stream.
pub async fn write_password<S: AsyncSocket>(stream: &mut S, password: &str) -> Result<(), Error> {
    #[cfg(feature = "crypto")]
    {
        use sha2::{Digest, Sha224};
        let hash = hex::encode(&Sha224::digest(password.as_bytes()));
        stream
            .write_all(hash.as_bytes())
            .await
            .map_err(|_| Error::Io("trojan: write failed"))?;
    }
    #[cfg(not(feature = "crypto"))]
    {
        let _ = password;
        return Err(Error::Unsupported("trojan: crypto feature not enabled"));
    }
    stream
        .write_all(CRLF)
        .await
        .map_err(|_| Error::Io("trojan: write failed"))
}

/// Read command byte + address + port + CRLF.
pub async fn read_request<S: AsyncSocket>(stream: &mut S) -> Result<(u8, Address, u16), Error> {
    let mut head = [0u8; 1]; // cmd
    read_exact(stream, &mut head).await?;
    let cmd = head[0];

    let (addr, port) = read_address(stream).await?;

    let mut crlf = [0u8; 2];
    read_exact(stream, &mut crlf).await?;
    if &crlf != CRLF {
        return Err(Error::Protocol("trojan: expected CRLF after address"));
    }

    Ok((cmd, addr, port))
}

/// Write command byte + address + port + CRLF.
pub async fn write_request<S: AsyncSocket>(
    stream: &mut S,
    cmd: u8,
    addr: &Address,
    port: u16,
) -> Result<(), Error> {
    stream
        .write_all(&[cmd])
        .await
        .map_err(|_| Error::Io("trojan: write failed"))?;
    write_address(stream, addr, port).await?;
    stream
        .write_all(CRLF)
        .await
        .map_err(|_| Error::Io("trojan: write failed"))
}

/// Read socks5-style address + port.
async fn read_address<S: AsyncSocket>(stream: &mut S) -> Result<(Address, u16), Error> {
    let mut atyp = [0u8; 1];
    read_exact(stream, &mut atyp).await?;

    let addr = match atyp[0] {
        ATYP_IPV4 => {
            let mut bytes = [0u8; 4];
            read_exact(stream, &mut bytes).await?;
            Address::Ipv4(bytes)
        }
        ATYP_IPV6 => {
            let mut bytes = [0u8; 16];
            read_exact(stream, &mut bytes).await?;
            Address::Ipv6(bytes)
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1];
            read_exact(stream, &mut len).await?;
            let mut domain = vec![0u8; len[0] as usize];
            read_exact(stream, &mut domain).await?;
            let domain = String::from_utf8(domain)
                .map_err(|_| Error::Protocol("trojan: invalid domain encoding"))?;
            Address::Domain(domain)
        }
        _ => return Err(Error::Protocol("trojan: unsupported address type")),
    };

    let mut port_bytes = [0u8; 2];
    read_exact(stream, &mut port_bytes).await?;
    let port = u16::from_be_bytes(port_bytes);

    Ok((addr, port))
}

/// Write socks5-style address + port.
async fn write_address<S: AsyncSocket>(
    stream: &mut S,
    addr: &Address,
    port: u16,
) -> Result<(), Error> {
    match addr {
        Address::Ipv4(bytes) => {
            stream
                .write_all(&[ATYP_IPV4])
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
            stream
                .write_all(bytes)
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
        }
        Address::Ipv6(bytes) => {
            stream
                .write_all(&[ATYP_IPV6])
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
            stream
                .write_all(bytes)
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
        }
        Address::Domain(domain) => {
            let b = domain.as_bytes();
            if b.is_empty() || b.len() > 255 {
                return Err(Error::Protocol("trojan: domain too long"));
            }
            stream
                .write_all(&[ATYP_DOMAIN, b.len() as u8])
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
            stream
                .write_all(b)
                .await
                .map_err(|_| Error::Io("trojan: write failed"))?;
        }
    }
    stream
        .write_all(&port.to_be_bytes())
        .await
        .map_err(|_| Error::Io("trojan: write failed"))
}

async fn read_exact<S: AsyncSocket>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Protocol("trojan: read failed"))?;
        if n == 0 {
            return Err(Error::Protocol("trojan: unexpected EOF"));
        }
        offset += n;
    }
    Ok(())
}

#[cfg(feature = "crypto")]
pub mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
