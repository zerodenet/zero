use std::io;
use std::net::{IpAddr, SocketAddr};

use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener as TokioTcpListener, TcpStream, UdpSocket};
use zero_traits::{
    AsyncSocket, DatagramSocket as DatagramSocketTrait, DnsResolver, IpAddress,
    TcpListener as TcpListenerTrait,
};

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

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
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

#[derive(Debug)]
pub struct TokioDatagramSocket {
    inner: UdpSocket,
}

impl TokioDatagramSocket {
    pub async fn bind(addr: &str) -> io::Result<Self> {
        UdpSocket::bind(addr).await.map(|inner| Self { inner })
    }

    pub async fn bind_addr(addr: SocketAddr) -> io::Result<Self> {
        UdpSocket::bind(addr).await.map(|inner| Self { inner })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub async fn recv_from_addr(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf).await
    }

    pub async fn send_to_addr(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        self.inner.send_to(buf, addr).await
    }
}

impl DatagramSocketTrait for TokioDatagramSocket {
    type Error = io::Error;

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, IpAddress, u16), Self::Error> {
        let (read, addr) = self.inner.recv_from(buf).await?;
        Ok((read, ip_addr_to_ip(addr.ip()), addr.port()))
    }

    async fn send_to(&self, buf: &[u8], addr: IpAddress, port: u16) -> Result<(), Self::Error> {
        self.inner
            .send_to(buf, socket_addr_from_ip(addr, port))
            .await
            .map(|_| ())
    }
}

fn socket_addr_from_ip(ip: IpAddress, port: u16) -> SocketAddr {
    match ip {
        IpAddress::V4(bytes) => SocketAddr::new(IpAddr::V4(bytes.into()), port),
        IpAddress::V6(bytes) => SocketAddr::new(IpAddr::V6(bytes.into()), port),
    }
}
