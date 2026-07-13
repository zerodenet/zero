use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

use super::InboundClientHello;

pub(super) fn parse_extensions(ext_data: &[u8], consumed: Vec<u8>) -> InboundClientHello {
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
                if ext_bytes.len() >= 5 && ext_bytes[2] == 0x00 {
                    let name_len = u16::from_be_bytes([ext_bytes[3], ext_bytes[4]]) as usize;
                    if 5 + name_len <= ext_bytes.len() {
                        if let Ok(name) = std::str::from_utf8(&ext_bytes[5..5 + name_len]) {
                            sni = Some(name.to_owned());
                        }
                    }
                }
            }
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

    InboundClientHello {
        sni,
        alpn,
        consumed,
    }
}

pub(super) async fn read_exact<R>(
    reader: &mut R,
    consumed: &mut Vec<u8>,
    dest: &mut [u8],
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    reader.read_exact(dest).await?;
    consumed.extend_from_slice(dest);
    Ok(())
}

pub(super) async fn skip_exact<R>(
    reader: &mut R,
    consumed: &mut Vec<u8>,
    len: usize,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut chunk = vec![0u8; len.min(256)];
    let mut remaining = len;
    while remaining > 0 {
        let to_read = remaining.min(chunk.len());
        reader.read_exact(&mut chunk[..to_read]).await?;
        consumed.extend_from_slice(&chunk[..to_read]);
        remaining -= to_read;
    }
    Ok(())
}
