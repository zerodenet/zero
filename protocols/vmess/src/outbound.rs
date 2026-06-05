use rand::Rng;
use zero_core::{Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, TcpTunnelProtocol};

use crate::crypto::{
    aead_decrypt, aead_encrypt, compute_auth_id, current_timestamp, derive_body_key_nonce,
    derive_cmd_key, derive_response_key_nonce, GCM_TAG_LEN,
};
use crate::shared::{read_exact, write_address, VmessCipher, AUTH_ID_LEN, CMD_TCP, VERSION};

#[derive(Debug, Clone, Copy)]
pub struct VmessOutbound;

impl VmessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vmess
    }

    pub async fn send_tcp_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<(), Error> {
        send_request(stream, session, uuid, cipher).await
    }

    pub async fn establish_tcp_tunnel<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<(), Error> {
        // 1. Send auth header
        send_request(stream, session, uuid, cipher).await?;

        // 2. Read and verify server response
        read_response(stream, uuid, cipher).await?;

        Ok(())
    }
}

/// Target parameters for VMess TCP tunnel.
#[derive(Debug, Clone, Copy)]
pub struct VmessTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub uuid: &'a [u8; 16],
    pub cipher: VmessCipher,
}

impl<'a> TcpTunnelProtocol<VmessTcpTunnelTarget<'a>> for VmessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VmessTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_tunnel(stream, target.session, target.uuid, target.cipher)
            .await
    }
}

async fn send_request<S: AsyncSocket>(
    stream: &mut S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<(), Error> {
    // 1. Derive cmd_key from UUID
    let cmd_key = derive_cmd_key(uuid, cipher.key_len());

    // 2. Build command body plaintext
    let timestamp = current_timestamp();
    let random_bytes = rand::rng().random::<[u8; 4]>();

    let mut command_body = Vec::new();
    command_body.extend_from_slice(&timestamp.to_be_bytes()); // [0:8] timestamp
    command_body.extend_from_slice(&random_bytes); // [8:12] random
    command_body.push(0x00); // [12] options
    command_body.push(0x00); // [13] padding_len (no padding)
    command_body.push(VERSION); // [14] version
    command_body.push(CMD_TCP); // [15] command (TCP)
    command_body.extend_from_slice(&session.port.to_be_bytes()); // [16:18] port
    write_address(&mut command_body, &session.target)?; // [18..] address

    // 3. Compute auth_id = HMAC(cmd_key, timestamp)[:16]
    let auth_id = compute_auth_id(&cmd_key, timestamp);

    // 4. Derive body key/nonce
    let (body_key_bytes, body_nonce_bytes) =
        derive_body_key_nonce(&cmd_key, &auth_id, cipher.key_len());

    // 5. Encrypt command body
    let encrypted = aead_encrypt(&body_key_bytes, &body_nonce_bytes, &command_body, cipher)?;
    let body_len = (encrypted.len() - GCM_TAG_LEN) as u16;

    // 6. Send: [auth_id:16][body_len:2 BE][encrypted_body]
    let mut packet = Vec::with_capacity(AUTH_ID_LEN + 2 + encrypted.len());
    packet.extend_from_slice(&auth_id);
    packet.extend_from_slice(&body_len.to_be_bytes());
    packet.extend_from_slice(&encrypted);

    stream
        .write_all(&packet)
        .await
        .map_err(|_| Error::Io("vmess: failed to write to socket"))?;

    Ok(())
}

async fn read_response<S: AsyncSocket>(
    stream: &mut S,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<(), Error> {
    let cmd_key = derive_cmd_key(uuid, cipher.key_len());

    // Read: [len:2 BE][encrypted_body][tag]
    let mut len_buf = [0u8; 2];
    read_exact(stream, &mut len_buf).await?;
    let body_len = u16::from_be_bytes(len_buf) as usize;
    if body_len > 256 {
        return Err(Error::Protocol("vmess response too large"));
    }

    let mut encrypted = vec![0u8; body_len + GCM_TAG_LEN];
    read_exact(stream, &mut encrypted).await?;

    // Reconstruct timestamp and auth_id the same way we did in send_request.
    let timestamp = current_timestamp();
    let auth_id = compute_auth_id(&cmd_key, timestamp);

    let (_body_key_bytes, _body_nonce_bytes) =
        derive_body_key_nonce(&cmd_key, &auth_id, cipher.key_len());

    // Derive response key from body_key
    let (resp_key, resp_nonce) =
        derive_response_key_nonce(&_body_key_bytes, &auth_id, cipher.key_len());

    // Decrypt response
    let plaintext = aead_decrypt(&resp_key, &resp_nonce, &encrypted, cipher)?;

    if plaintext.is_empty() || plaintext[0] != 0x00 {
        return Err(Error::Protocol("vmess server rejected connection"));
    }

    Ok(())
}
