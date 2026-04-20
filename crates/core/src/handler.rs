use zero_traits::AsyncSocket;

use crate::{Error, Session};

pub trait InboundHandler<S>
where
    S: AsyncSocket,
{
    async fn handshake(&self, stream: &mut S) -> Result<Session, Error>;
}

pub trait OutboundHandler<S>
where
    S: AsyncSocket,
{
    async fn connect(&self, session: &Session) -> Result<S, Error>;
}
