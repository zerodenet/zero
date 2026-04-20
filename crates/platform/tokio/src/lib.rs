use std::io;
use std::net::{IpAddr, SocketAddr};

use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener as TokioTcpListener, TcpStream};
use zero_traits::{AsyncSocket, DnsResolver, IpAddress, TcpListener as TcpListenerTrait};

#[derive(Debug)]
pub struct TokioSocket {
    inner: TcpStream,
}

impl TokioSocket {
    pub fn new(inner: TcpStream) -> Self {
        Self { inner }
    }

    pub async fn connect(addr: &str) -> io::Result<Self> {
        TcpStream::connect(addr).await.map(Self::new)
    }

    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        TcpStream::connect(addr).await.map(Self::new)
    }

    pub fn into_inner(self) -> TcpStream {
        self.inner
    }
}

impl AsyncSocket for TokioSocket {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.inner.shutdown().await
    }
}

#[derive(Debug)]
pub struct TokioListener {
    inner: TokioTcpListener,
}

impl TokioListener {
    pub async fn bind(addr: &str) -> io::Result<Self> {
        TokioTcpListener::bind(addr)
            .await
            .map(|inner| Self { inner })
    }

    pub async fn accept(&self) -> io::Result<(TokioSocket, Option<IpAddress>)> {
        <Self as TcpListenerTrait>::accept(self).await
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl TcpListenerTrait for TokioListener {
    type Stream = TokioSocket;
    type Error = io::Error;

    async fn accept(&self) -> Result<(Self::Stream, Option<IpAddress>), Self::Error> {
        let (stream, remote_addr) = self.inner.accept().await?;

        Ok((
            TokioSocket::new(stream),
            Some(socket_addr_to_ip(remote_addr)),
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokioResolver;

impl DnsResolver for TokioResolver {
    type Error = io::Error;

    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddress>, Self::Error> {
        let mut resolved = Vec::new();

        for addr in lookup_host((domain, 0)).await? {
            resolved.push(ip_addr_to_ip(addr.ip()));
        }

        Ok(resolved)
    }
}

fn socket_addr_to_ip(addr: SocketAddr) -> IpAddress {
    ip_addr_to_ip(addr.ip())
}

fn ip_addr_to_ip(addr: IpAddr) -> IpAddress {
    match addr {
        IpAddr::V4(addr) => IpAddress::V4(addr.octets()),
        IpAddr::V6(addr) => IpAddress::V6(addr.octets()),
    }
}

pub async fn relay_bidirectional(left: TokioSocket, right: TokioSocket) -> io::Result<(u64, u64)> {
    let mut left = left.into_inner();
    let mut right = right.into_inner();

    copy_bidirectional(&mut left, &mut right).await
}
