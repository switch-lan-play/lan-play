use super::EthernetInterface;
use tokio::io::{self, AsyncRead};
use std::pin::Pin;
use std::task::{Context, Poll};

pub enum Socket {
    TcpSocket(TcpSocket),
    UdpSocket(UdpSocket),
}

pub struct TcpSocket {
}

pub struct UdpSocket {
}

pub struct SocketLeaf {
    
}

impl TcpSocket {
    
}

// impl AsyncRead for TcpSocket {
//     fn poll_read(
//         self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<io::Result<usize>> {

//     }
// }
