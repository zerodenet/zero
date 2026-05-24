use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

pub const VERSION: u8 = 0x01;
pub const CMD_TCP: u8 = 0x01;
pub const CMD_UDP: u8 = 0x02;

const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x02;
const ATYP_IPV6: u8 = 0x03;

pub const AUTH_ID_LEN: usize = 16;
pub const GCM_TAG_LEN: usize = 16;

/// AEAD cipher variants for VMess header encryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmessCipher {
    Aes128Gcm,
    Aes256Gcm,
    Chacha20Poly1305,
}

impl VmessCipher {
    pub fn key_len(self) -> usize {
        match self {
            VmessCipher::Aes128Gcm => 16,
            VmessCipher::Aes256Gcm => 32,
            VmessCipher::Chacha20Poly1305 => 32,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            VmessCipher::Aes128Gcm => "aes-128-gcm",
            VmessCipher::Aes256Gcm => "aes-256-gcm",
            VmessCipher::Chacha20Poly1305 => "chacha20-poly1305",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aes-128-gcm" => Some(VmessCipher::Aes128Gcm),
            "aes-256-gcm" => Some(VmessCipher::Aes256Gcm),
            "chacha20-poly1305" => Some(VmessCipher::Chacha20Poly1305),
            _ => None,
        }
    }

    pub(crate) fn aead_algorithm(self) -> &'static ring::aead::Algorithm {
        match self {
            VmessCipher::Aes128Gcm => &ring::aead::AES_128_GCM,
            VmessCipher::Aes256Gcm => &ring::aead::AES_256_GCM,
            VmessCipher::Chacha20Poly1305 => &ring::aead::CHACHA20_POLY1305,
        }
    }
}

pub(crate) async fn read_exact<S: AsyncSocket>(
    stream: &mut S,
    buf: &mut [u8],
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
        if n == 0 {
            return Err(Error::Io("vmess: unexpected EOF while reading socket"));
        }
        offset += n;
    }
    Ok(())
}

pub(crate) fn write_address(buf: &mut Vec<u8>, address: &Address) -> Result<(), Error> {
    match address {
        Address::Ipv4(addr) => {
            buf.push(ATYP_IPV4);
            buf.extend_from_slice(addr);
        }
        Address::Domain(domain) => {
            let domain_bytes = domain.as_bytes();
            if domain_bytes.len() > 255 {
                return Err(Error::Protocol("vmess domain too long (>255)"));
            }
            buf.push(ATYP_DOMAIN);
            buf.push(domain_bytes.len() as u8);
            buf.extend_from_slice(domain_bytes);
        }
        Address::Ipv6(addr) => {
            buf.push(ATYP_IPV6);
            buf.extend_from_slice(addr);
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn read_address<S: AsyncSocket>(
    stream: &mut S,
    atyp: u8,
) -> Result<Address, Error> {
    match atyp {
        ATYP_IPV4 => {
            let mut addr = [0u8; 4];
            read_exact(stream, &mut addr).await?;
            Ok(Address::Ipv4(addr))
        }
        ATYP_DOMAIN => {
            let mut len_buf = [0u8; 1];
            read_exact(stream, &mut len_buf).await?;
            let len = len_buf[0] as usize;
            let mut domain = vec![0u8; len];
            read_exact(stream, &mut domain).await?;
            let s = String::from_utf8(domain)
                .map_err(|_| Error::Protocol("vmess domain is not valid utf-8"))?;
            Ok(Address::Domain(s))
        }
        ATYP_IPV6 => {
            let mut addr = [0u8; 16];
            read_exact(stream, &mut addr).await?;
            Ok(Address::Ipv6(addr))
        }
        _ => Err(Error::Protocol("vmess unknown address type")),
    }
}

pub fn parse_uuid(input: &str) -> Result<[u8; 16], Error> {
    let hex = input.replace('-', "");
    if hex.len() != 32 {
        return Err(Error::Protocol("vmess uuid must be 32 hex characters"));
    }
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::Protocol("vmess uuid contains invalid hex characters"))?;
    }
    Ok(bytes)
}
