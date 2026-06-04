use tokio::sync::mpsc;

use vless::{VlessInbound, VlessOutbound, VlessUser, VlessUserStore};
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
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
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
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        for &b in buf {
            let _ = self.tx.send(b);
        }
        Ok(())
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
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
        async move { inbound.accept_mux_header(&mut server, &auth).await }
    });

    let outbound = VlessOutbound;
    let mut client = client;
    let mux = outbound.establish_mux(&mut client, &id).await.unwrap();
    drop(mux);

    assert!(server_h.await.unwrap().is_ok());
}
