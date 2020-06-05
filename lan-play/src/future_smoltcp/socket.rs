use tokio::io::{self, AsyncRead, AsyncWrite, StreamReader, stream_reader};
use tokio::sync::mpsc;
pub use smoltcp::socket::SocketHandle;
use futures::stream::{BoxStream, StreamExt};
use futures::future::{BoxFuture, FutureExt};
use std::{pin::Pin, task::{Poll, Context}};
use bytes::Bytes;
use super::OutPacket;

pub type Packet = Bytes;

#[derive(Debug)]
pub enum Socket {
    Tcp(TcpSocket),
    Udp(UdpSocket),
}

pub struct TcpSocket {
    handle: SocketHandle,
    reader: StreamReader<BoxStream<'static, std::io::Result<Packet>>, Packet>,
    writer: mpsc::Sender<OutPacket>,
    sending: Option<BoxFuture<'static, Result<(), mpsc::error::SendError<OutPacket>>>>,
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
    pub(super) fn new(handle: SocketHandle, writer: mpsc::Sender<OutPacket>) -> (Self, SocketLeaf) {
        let (tx, rx) = mpsc::channel(10);
        let reader = stream_reader(
            rx.map(|x| Ok(x)).boxed()
        );
        (TcpSocket {
            handle,
            reader,
            writer,
            sending: None,
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

impl AsyncWrite for TcpSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match futures::ready!(self.writer.poll_ready(cx)) {
            Ok(_) => {
                let handle = self.handle;
                Poll::Ready(
                    self.writer
                        .try_send((handle, buf.to_vec()))
                        .map(|_| buf.len())
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
                )
            },
            Err(e) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e))),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        // TODO: implement shutdown
        Poll::Ready(Ok(()))
    }
}
