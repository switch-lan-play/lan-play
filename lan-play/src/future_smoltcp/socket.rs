use super::{
    raw_udp::{parse_udp_owned, ChecksumCapabilities, OwnedUdp},
    reactor::Source,
    NetReactor, OutPacket,
};
use bytes::Bytes;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{BoxStream, StreamExt};
pub use smoltcp::socket::{self, SocketHandle, SocketRef};
use smoltcp::Error;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    mem::replace,
};
use tokio::io::{self, stream_reader, AsyncRead, AsyncWrite, StreamReader};
use tokio::sync::mpsc;

pub type Packet = Bytes;

#[derive(Debug)]
pub enum Socket {
    Tcp(TcpSocket),
    Udp(UdpSocket),
}

pub struct TcpListener {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
}

pub struct TcpSocket {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
}

pub struct UdpSocket {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
}

pub struct SocketLeaf {
    tx: mpsc::Sender<Packet>,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.reactor.remove(&self.handle)
    }
}

fn map_err(e: smoltcp::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

impl TcpListener {
    pub(super) async fn new(reactor: NetReactor) -> TcpListener {
        let mut set = reactor.lock_set().await;
        let handle = set.new_tcp_socket();
        drop(set);

        let source = reactor.insert(handle);

        TcpListener {
            handle,
            reactor,
            source,
        }
    }
    pub async fn accept(&mut self) -> io::Result<TcpSocket> {
        loop {
            {
                let mut set = self.reactor.lock_set().await;
                let socket = set.as_set_mut().get::<socket::TcpSocket>(self.handle);
                if socket.can_send() {
                    drop(socket);
                    drop(set);
                    return Ok(TcpSocket::new(self).await)
                }
            }
            self.source.writable(&self.reactor).await?;
        }
    }
}

impl UdpSocket {
    pub(super) async fn new(reactor: NetReactor) -> UdpSocket {
        let mut set = reactor.lock_set().await;
        let handle = set.new_raw_socket();
        drop(set);

        let source = reactor.insert(handle);

        UdpSocket {
            handle,
            reactor,
            source,
        }
    }
    pub async fn recv(&mut self) -> io::Result<OwnedUdp> {
        loop {
            {
                let mut set = self.reactor.lock_set().await;
                let mut socket = set.as_set_mut().get::<socket::RawSocket>(self.handle);
                if socket.can_recv() {
                    return socket
                        .recv()
                        .map(|p| parse_udp_owned(p, &ChecksumCapabilities::default()))
                        .and_then(|x| x)
                        .map_err(map_err);
                }
            }
            self.source.readable(&self.reactor).await?;
        }
    }
    pub async fn send(&mut self, data: OwnedUdp) -> io::Result<()> {
        loop {
            {
                let mut set = self.reactor.lock_set().await;
                let mut socket = set
                    .as_set_mut()
                    .get::<smoltcp::socket::RawSocket>(self.handle);
                if socket.can_send() {
                    match socket.send_slice(&data.to_raw()) {
                        Err(Error::Exhausted) => {}
                        res => return res.map_err(map_err),
                    }
                }
            }
            self.source.writable(&self.reactor).await?;
        }
    }
}

impl SocketLeaf {
    pub async fn send<P: Into<Packet>>(&mut self, packet: P) {
        self.tx.send(packet.into()).await.unwrap()
    }
    pub fn try_send<P: Into<Packet>>(&mut self, packet: P) {
        self.tx
            .try_send(packet.into())
            .expect("FIXME try send failed");
    }
}

impl TcpSocket {
    async fn new(listener: &mut TcpListener) -> TcpSocket {
        let mut reactor = listener.reactor.clone();
        let mut set = reactor.lock_set().await;
        let handle = set.new_tcp_socket();
        drop(set);

        let source = reactor.insert(handle);

        TcpSocket {
            handle: replace(&mut listener.handle, handle),
            reactor,
            source: replace(&mut listener.source, source),
        }
    }
}

impl Into<Socket> for TcpSocket {
    fn into(self) -> Socket {
        Socket::Tcp(self)
    }
}

impl std::fmt::Debug for TcpSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpSocket")
            .field("handle", &self.handle)
            .finish()
    }
}

impl std::fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpSocket")
            .field("handle", &self.handle)
            .finish()
    }
}

impl AsyncRead for TcpSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // Pin::new(&mut self.reader).poll_read(cx, buf)
        todo!();
    }
}

impl AsyncWrite for TcpSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        todo!();
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        // TODO: implement shutdown
        Poll::Ready(Ok(()))
    }
}
