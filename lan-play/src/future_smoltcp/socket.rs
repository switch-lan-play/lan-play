use tokio::io::{self, AsyncRead, StreamReader, stream_reader};
use tokio::sync::mpsc;
use smoltcp::socket::SocketHandle;
use futures::stream::{BoxStream, StreamExt};
use std::{pin::Pin, collections::VecDeque, task::{Poll, Context}};

pub type Packet = VecDeque<u8>;

#[derive(Debug)]
pub enum Socket {
    Tcp(TcpSocket),
    Udp(UdpSocket),
}

pub struct TcpSocket {
    handle: SocketHandle,
    reader: StreamReader<BoxStream<'static, std::io::Result<Packet>>, Packet>,
}

pub struct UdpSocket {
    handle: SocketHandle,
}

pub struct SocketLeaf {
    tx: mpsc::Sender<Packet>,
}

impl SocketLeaf {
    pub async fn send<P: Into<Packet>>(&mut self, packet: P) {
        self.tx.send(packet.into()).await.unwrap()
    }
}

impl TcpSocket {
    pub(super) fn new(handle: SocketHandle) -> (Self, SocketLeaf) {
        let (tx, rx) = mpsc::channel(10);
        let reader = stream_reader(
            rx.map(|x| Ok(x)).boxed()
        );
        (TcpSocket {
            handle,
            reader,
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
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
    fn poll_read_buf<BM: bytes::BufMut>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut BM,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.reader).poll_read_buf(cx, buf)
    }
}
