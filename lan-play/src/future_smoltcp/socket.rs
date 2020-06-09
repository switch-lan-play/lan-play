use tokio::io::{self, AsyncRead, AsyncWrite, StreamReader, stream_reader};
use tokio::sync::mpsc;
pub use smoltcp::socket::SocketHandle;
use futures::stream::{BoxStream, StreamExt};
use futures::future::{BoxFuture, FutureExt};
use std::{pin::Pin, task::{Poll, Context}, sync::Arc};
use bytes::Bytes;
use super::{NetReactor, OutPacket, reactor::Source};

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
}

pub struct UdpSocket {
    handle: SocketHandle,
}

pub struct SocketLeaf {
    tx: mpsc::Sender<Packet>,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.reactor.remove(&self.handle)
    }
}

impl TcpListener {
    pub(super) fn new(reactor: NetReactor) -> TcpListener {
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
    pub async fn accept() {
        loop {
            self.reactor.read
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
    pub(super) fn new(handle: SocketHandle, reactor: NetReactor) -> (Self, SocketLeaf) {
        let (tx, rx) = mpsc::channel(10);
        let reader = stream_reader(
            rx.map(|x| Ok(x)).boxed()
        );
        (TcpSocket {
            handle,
            reactor,
        }, SocketLeaf {
            tx
        })
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
