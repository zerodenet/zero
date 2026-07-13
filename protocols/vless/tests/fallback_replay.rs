use zero_traits::AsyncSocket;

#[derive(Debug, Default)]
struct MemorySocket {
    written: Vec<u8>,
}

impl AsyncSocket for MemorySocket {
    type Error = core::convert::Infallible;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(0)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.written.extend_from_slice(buf);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn matching_alpn_replays_the_recorded_prefix_and_returns_the_client_stream() {
    let client_stream = String::from("client-stream");
    let replay = vless::inbound::fallback_replay_for_alpns(
        Some("http/1.1"),
        ["h2", "http/1.1"],
        client_stream,
        b"recorded-client-prefix".to_vec(),
    )
    .into_transport_parts()
    .expect("matching ALPN should select fallback replay");
    let mut upstream = MemorySocket::default();

    let returned_stream = replay
        .replay_to_upstream(&mut upstream)
        .await
        .expect("in-memory replay cannot fail");

    assert_eq!(upstream.written, b"recorded-client-prefix");
    assert_eq!(returned_stream, "client-stream");
}

#[test]
fn non_matching_alpn_preserves_the_stream_and_prefix_for_tls_acceptance() {
    let decision = vless::inbound::fallback_replay_for_alpns(
        Some("http/1.1"),
        ["h2"],
        String::from("client-stream"),
        b"tls-client-hello-prefix".to_vec(),
    );

    let (stream, prefix) = match decision.into_transport_parts() {
        Ok(_) => panic!("non-matching ALPN should continue TLS acceptance"),
        Err(parts) => parts,
    };

    assert_eq!(stream, "client-stream");
    assert_eq!(prefix, b"tls-client-hello-prefix");
}
