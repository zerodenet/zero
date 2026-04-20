use zero_core::Error;
use zero_traits::AsyncSocket;

pub(crate) const SOCKS5_VERSION: u8 = 0x05;
pub(crate) const METHOD_NO_AUTH: u8 = 0x00;
pub(crate) const METHOD_NOT_ACCEPTABLE: u8 = 0xff;

pub(crate) const CMD_CONNECT: u8 = 0x01;

pub(crate) const ATYP_IPV4: u8 = 0x01;
pub(crate) const ATYP_DOMAIN: u8 = 0x03;
pub(crate) const ATYP_IPV6: u8 = 0x04;

pub(crate) const REP_SUCCEEDED: u8 = 0x00;
pub(crate) const REP_GENERAL_FAILURE: u8 = 0x01;
pub(crate) const REP_CONNECTION_NOT_ALLOWED: u8 = 0x02;
pub(crate) const REP_HOST_UNREACHABLE: u8 = 0x04;
pub(crate) const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub(crate) const REP_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5Reply {
    Succeeded,
    GeneralFailure,
    ConnectionNotAllowed,
    HostUnreachable,
    CommandNotSupported,
    AddressTypeNotSupported,
}

impl Socks5Reply {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Succeeded => REP_SUCCEEDED,
            Self::GeneralFailure => REP_GENERAL_FAILURE,
            Self::ConnectionNotAllowed => REP_CONNECTION_NOT_ALLOWED,
            Self::HostUnreachable => REP_HOST_UNREACHABLE,
            Self::CommandNotSupported => REP_COMMAND_NOT_SUPPORTED,
            Self::AddressTypeNotSupported => REP_ADDRESS_TYPE_NOT_SUPPORTED,
        }
    }
}

pub(crate) async fn write_reply<S>(stream: &mut S, reply: Socks5Reply) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let response = [
        SOCKS5_VERSION,
        reply.code(),
        0x00,
        ATYP_IPV4,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    stream
        .write_all(&response)
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 response"))
}

pub(crate) async fn read_exact<S>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut offset = 0;

    while offset < buf.len() {
        let read = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("failed to read from socket"))?;

        if read == 0 {
            return Err(Error::Io("unexpected EOF while reading socket"));
        }

        offset += read;
    }

    Ok(())
}
