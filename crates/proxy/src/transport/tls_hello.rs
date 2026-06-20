//! Parse TLS ClientHello without consuming the underlying stream.
//!
//! Reads just enough bytes to extract SNI and ALPN extensions, then returns
//! both the extracted metadata and a copy of the consumed bytes for replay.

use std::io;

/// Parsed TLS ClientHello metadata.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct ClientHelloInfo {
    /// Server Name Indication (hostname).
    pub sni: Option<String>,
    /// Negotiated ALPN protocols advertised by the client.
    pub alpn: Vec<String>,
    /// All bytes consumed while parsing (ready for replay).
    pub consumed: Vec<u8>,
}

/// Errors returned by ClientHello parsing.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum HelloError {
    Io(io::Error),
    NotTls,
    Truncated,
}

impl From<io::Error> for HelloError {
    fn from(e: io::Error) -> Self {
        HelloError::Io(e)
    }
}

/// Peek at a TLS ClientHello from `reader`.
///
/// Reads and buffers the TLS record + handshake headers and extension data.
/// On success the caller can replay `info.consumed` to whichever path is
/// chosen (TLS acceptor or fallback).
pub(crate) async fn peek_client_hello<R>(reader: &mut R) -> Result<ClientHelloInfo, HelloError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut buf = Vec::with_capacity(512);

    // ── TLS record header (5 bytes) ──
    // [content_type(1)][version(2)][length(2)]
    let mut record_hdr = [0u8; 5];
    reader.read_exact(&mut record_hdr).await?;
    if record_hdr[0] != 0x16 {
        return Err(HelloError::NotTls);
    }
    let _record_len = u16::from_be_bytes([record_hdr[3], record_hdr[4]]) as usize;
    buf.extend_from_slice(&record_hdr);

    // ── Handshake header (4 bytes) ──
    let mut hshake_hdr = [0u8; 4];
    reader.read_exact(&mut hshake_hdr).await?;
    if hshake_hdr[0] != 0x01 {
        return Err(HelloError::NotTls);
    }
    let _hshake_len = u24_from_be(&hshake_hdr[1..]);
    buf.extend_from_slice(&hshake_hdr);

    // ── ClientHello fixed fields ──
    // version(2) + random(32) + session_id_len(1) + session_id(v)
    // + cipher_suites_len(2) + cipher_suites(v)
    // + compression_len(1) + compression(v)
    // Read in chunks until we reach extensions
    let mut fixed = [0u8; 38]; // version(2) + random(32) + sid_len(1) + enough to start
    let fixed_needed = 2 + 32 + 1;
    read_exact_buf(reader, &mut buf, &mut fixed[..fixed_needed]).await?;

    let session_id_len = fixed[34] as usize;
    skip_exact(reader, &mut buf, session_id_len).await?;

    // Read cipher_suites_len
    let mut cs_len_buf = [0u8; 2];
    read_exact_buf(reader, &mut buf, &mut cs_len_buf).await?;
    let cs_len = u16::from_be_bytes(cs_len_buf) as usize;
    skip_exact(reader, &mut buf, cs_len).await?;

    // Read compression_methods_len
    let mut cm_len_buf = [0u8; 1];
    read_exact_buf(reader, &mut buf, &mut cm_len_buf).await?;
    let cm_len = cm_len_buf[0] as usize;
    skip_exact(reader, &mut buf, cm_len).await?;

    // ── Extensions length (2 bytes) ──
    let mut ext_len_buf = [0u8; 2];
    // Extensions length may be zero
    match reader.read_exact(&mut ext_len_buf).await {
        Ok(_) => {
            buf.extend_from_slice(&ext_len_buf);
        }
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(ClientHelloInfo {
                consumed: buf,
                ..Default::default()
            });
        }
        Err(e) => return Err(HelloError::Io(e)),
    }
    let ext_len = u16::from_be_bytes(ext_len_buf) as usize;

    // Don't read more than 8 KiB of extensions (safety limit)
    let ext_len = ext_len.min(8192);
    let mut ext_data = vec![0u8; ext_len];
    read_exact_buf(reader, &mut buf, &mut ext_data).await?;

    // ── Parse extensions ──
    let info = parse_extensions(&ext_data, buf);

    Ok(info)
}

fn parse_extensions(ext_data: &[u8], consumed: Vec<u8>) -> ClientHelloInfo {
    let mut sni = None;
    let mut alpn = Vec::new();
    let mut offset = 0;

    while offset + 4 <= ext_data.len() {
        let ext_type = u16::from_be_bytes([ext_data[offset], ext_data[offset + 1]]);
        let ext_len = u16::from_be_bytes([ext_data[offset + 2], ext_data[offset + 3]]) as usize;
        offset += 4;

        if offset + ext_len > ext_data.len() {
            break;
        }
        let ext_bytes = &ext_data[offset..offset + ext_len];

        match ext_type {
            0x0000 => {
                // SNI: [list_len(2)][type(1)][len(2)][name(N)]
                if ext_bytes.len() >= 5 && ext_bytes[2] == 0x00 {
                    let name_len = u16::from_be_bytes([ext_bytes[3], ext_bytes[4]]) as usize;
                    if 5 + name_len <= ext_bytes.len() {
                        if let Ok(name) = std::str::from_utf8(&ext_bytes[5..5 + name_len]) {
                            sni = Some(name.to_owned());
                        }
                    }
                }
            }
            // ALPN: [alpn_ext_len(2)][proto_list_len(2)][len(1)][proto(N)]...
            0x0010 if ext_bytes.len() >= 4 => {
                let list_len = u16::from_be_bytes([ext_bytes[2], ext_bytes[3]]) as usize;
                let mut pos = 4;
                while pos < ext_bytes.len() && pos < 4 + list_len {
                    let proto_len = ext_bytes[pos] as usize;
                    pos += 1;
                    if pos + proto_len <= ext_bytes.len() {
                        if let Ok(proto) = std::str::from_utf8(&ext_bytes[pos..pos + proto_len]) {
                            alpn.push(proto.to_owned());
                        }
                        pos += proto_len;
                    } else {
                        break;
                    }
                }
            }
            _ => {}
        }
        offset += ext_len;
    }

    ClientHelloInfo {
        sni,
        alpn,
        consumed,
    }
}

fn u24_from_be(bytes: &[u8]) -> usize {
    ((bytes[0] as usize) << 16) | ((bytes[1] as usize) << 8) | (bytes[2] as usize)
}

async fn read_exact_buf<R>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    dest: &mut [u8],
) -> Result<(), HelloError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    reader.read_exact(dest).await?;
    buf.extend_from_slice(dest);
    Ok(())
}

async fn skip_exact<R>(reader: &mut R, buf: &mut Vec<u8>, len: usize) -> Result<(), HelloError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut chunk = vec![0u8; len.min(256)];
    let mut remaining = len;
    while remaining > 0 {
        let to_read = remaining.min(chunk.len());
        reader.read_exact(&mut chunk[..to_read]).await?;
        buf.extend_from_slice(&chunk[..to_read]);
        remaining -= to_read;
    }
    Ok(())
}
