use super::EthernetInterface;
use tokio::io::{self, AsyncRead};
use std::pin::Pin;
use std::task::{Context, Poll};
use smoltcp::socket::SocketHandle;

pub enum Socket {
    TcpSocket(TcpSocket),
    UdpSocket(UdpSocket),
}

#[derive(Debug)]
pub struct TcpSocket {
    handle: SocketHandle,
}

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

// impl AsyncRead for TcpSocket {
//     fn poll_read(
//         self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<io::Result<usize>> {

//     }
// }
