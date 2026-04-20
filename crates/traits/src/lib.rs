#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddress {
    V4([u8; 4]),
    V6([u8; 16]),
}

pub trait AsyncSocket: Send + Sync + Unpin {
    type Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
    async fn shutdown(&mut self) -> Result<(), Self::Error>;
}

pub trait TcpListener: Send + Sync {
    type Stream: AsyncSocket;
    type Error;

    async fn accept(&self) -> Result<(Self::Stream, Option<IpAddress>), Self::Error>;
}

pub trait DatagramSocket: Send + Sync + Unpin {
    type Error;

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, IpAddress, u16), Self::Error>;
    async fn send_to(&self, buf: &[u8], addr: IpAddress, port: u16) -> Result<(), Self::Error>;
}

pub trait DnsResolver: Send + Sync {
    type Error;

    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddress>, Self::Error>;
}

pub trait TlsConnector: Send + Sync {}

pub trait TlsAcceptor: Send + Sync {}

pub trait CryptoProvider: Send + Sync {}

pub trait TimeProvider: Send + Sync {
    fn now_unix_seconds(&self) -> u64;
}

pub trait Allocator: Send + Sync {}
