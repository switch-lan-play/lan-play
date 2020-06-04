use super::EthernetInterface;
use tokio::io::{self, AsyncRead};
use std::pin::Pin;
use std::task::{Context, Poll};
use smoltcp::socket::SocketHandle;

#[derive(Debug)]
pub enum Socket {
    Tcp(TcpSocket),
    Udp(UdpSocket),
}

#[derive(Debug)]
pub struct TcpSocket {
    handle: SocketHandle,
}

#[derive(Debug)]
pub struct UdpSocket {
}

pub struct SocketLeaf {
    
}

impl TcpSocket {
    pub(super) fn new(handle: SocketHandle) -> Self {
        TcpSocket {
            handle,
        }
    }
}

impl Into<Socket> for TcpSocket {
    fn into(self) -> Socket {
        Socket::Tcp(self)
    }
}

// impl AsyncRead for TcpSocket {
//     fn poll_read(
//         self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<io::Result<usize>> {

//     }
// }
