#![cfg(feature = "reality")]

use tokio::sync::mpsc;
use vless::inbound::{VlessInbound, VlessUser, VlessUserStore};
use vless::mux_pool::establish_outbound_mux_connection;
use zero_traits::AsyncSocket;

const USER_ID: &str = "11111111-2222-3333-4444-555555555555";

fn pair() -> (MockSocket, MockSocket) {
    let (a_tx, a_rx) = mpsc::unbounded_channel::<u8>();
    let (b_tx, b_rx) = mpsc::unbounded_channel::<u8>();
    (
        MockSocket { rx: a_rx, tx: b_tx },
        MockSocket { rx: b_rx, tx: a_tx },
    )
}

struct MockSocket {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl AsyncSocket for MockSocket {
    type Error = ();
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move {
            // Read at least one byte, then as many more as available
            let first = self.rx.recv().await.ok_or(())?;
            buf[0] = first;
            let mut n = 1;
            while n < buf.len() {
                match self.rx.try_recv() {
                    Ok(b) => {
                        buf[n] = b;
                        n += 1;
                    }
                    Err(_) => break,
                }
            }
            Ok(n)
        }
    }
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            for &b in buf {
                let _ = self.tx.send(b);
            }
            Ok(())
        }
    }
    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { Ok(()) }
    }
}

struct TestUsers([u8; 16]);
impl VlessUserStore for TestUsers {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        if id == &self.0 {
            Some(VlessUser::new())
        } else {
            None
        }
    }
}

fn uuid() -> [u8; 16] {
    vless::parse_uuid(USER_ID).unwrap()
}

#[tokio::test]
async fn mux_handshake_establishes() {
    let (client, server) = pair();
    let id = uuid();

    let server_h = tokio::spawn({
        let inbound = VlessInbound;
        let auth = TestUsers(id);
        let mut server = server;
        async move { inbound.handshake_mux_with_auth(&mut server, &auth).await }
    });

    let mut client = client;
    establish_outbound_mux_connection(&mut client, &id)
        .await
        .unwrap();

    let session = server_h.await.unwrap().unwrap();
    assert!(VlessInbound::is_mux_session(&session));
}
