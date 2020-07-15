use super::{
    raw_udp::{parse_udp_owned, endpoint2socketaddr, ChecksumCapabilities, OwnedUdp},
    reactor::Source,
    NetReactor,
};
pub use smoltcp::socket::{self, SocketHandle, SocketRef};

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    mem::replace,
    future::Future,
    net::{SocketAddr},
};
use crate::rt::{AsyncRead, AsyncWrite, io};

pub struct TcpListener {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
}

pub struct TcpSocket {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
}

pub struct UdpSocket {
    handle: SocketHandle,
    reactor: NetReactor,
    source: Arc<Source>,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.reactor.remove(&self.handle);
        let mut set = self.reactor.lock_set();
        set.remove(self.handle);
    }
}

fn map_err(e: smoltcp::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

impl TcpListener {
    pub(super) async fn new(reactor: NetReactor) -> TcpListener {
        let mut set = reactor.lock_set();
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
                let mut set = self.reactor.lock_set();
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
        let mut set = reactor.lock_set();
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
                let mut set = self.reactor.lock_set();
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
                let mut set = self.reactor.lock_set();
                let mut socket = set
                    .as_set_mut()
                    .get::<smoltcp::socket::RawSocket>(self.handle);
                if socket.can_send() {
                    let r = socket.send_slice(&data.to_raw())
                        .map_err(map_err);
                    self.reactor.notify();
                    return r;
                }
            }
            self.source.writable(&self.reactor).await?;
        }
    }
}

impl TcpSocket {
    async fn new(listener: &mut TcpListener) -> TcpSocket {
        let reactor = listener.reactor.clone();
        let mut set = reactor.lock_set();
        let socket = set.as_set_mut().get::<socket::TcpSocket>(listener.handle);
        let local_addr = endpoint2socketaddr(&socket.local_endpoint());
        let peer_addr = endpoint2socketaddr(&socket.remote_endpoint());
        drop(socket);
        let handle = set.new_tcp_socket();
        drop(set);

        let source = reactor.insert(handle);

        TcpSocket {
            reactor,
            handle: replace(&mut listener.handle, handle),
            source: replace(&mut listener.source, source),
            local_addr,
            peer_addr,
        }
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local_addr)
    }
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.peer_addr)
    }
    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            {
                let mut set = self.reactor.lock_set();
                let mut socket = set.as_set_mut().get::<socket::TcpSocket>(self.handle);
                if !socket.is_open() {
                    return Ok(0);
                }
                if socket.can_recv() {
                    return socket
                        .recv_slice(buf)
                        .map_err(map_err);
                }
            }
            self.source.readable(&self.reactor).await?;
        }
    }
    pub async fn send(&mut self, data: &[u8]) -> io::Result<usize> {
        loop {
            {
                let mut set = self.reactor.lock_set();
                let mut socket = set
                    .as_set_mut()
                    .get::<smoltcp::socket::TcpSocket>(self.handle);
                if socket.can_send() {
                    let r = socket.send_slice(data)
                        .map_err(map_err);
                    self.reactor.notify();
                    return r;
                }
            }
            self.source.writable(&self.reactor).await?;
        }
    }
    pub async fn shutdown(&mut self) -> io::Result<()> {
        let mut set = self.reactor.lock_set();
        let mut socket = set
            .as_set_mut()
            .get::<smoltcp::socket::TcpSocket>(self.handle);
        socket.close();
        Ok(())
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        self.reactor.remove(&self.handle);
        let mut set = self.reactor.lock_set();
        set.remove(self.handle);
        log::trace!("Drop TcpSocket {:?}", self.handle);
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
        let fut = self.recv(buf);
        futures::pin_mut!(fut);
        fut.poll(cx)
    }
}

impl AsyncWrite for TcpSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let fut = self.send(buf);
        futures::pin_mut!(fut);
        fut.poll(cx)
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    #[cfg(feature = "tokio")]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let fut = self.shutdown();
        futures::pin_mut!(fut);
        fut.poll(cx)
    }
    #[cfg(feature = "async_std")]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let fut = self.shutdown();
        futures::pin_mut!(fut);
        fut.poll(cx)
    }
}
