use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer, SocketSet, SocketHandle},
    wire::{IpCidr, IpAddress}
};
use super::interface::{RawsockInterface};

pub struct TcpListener {
    handle: Option<SocketHandle>,
}

impl<'a> TcpListener {
    pub async fn new(interf: &RawsockInterface) -> TcpListener {
        TcpListener {
            handle: None
        }
    }
    pub async fn next(&mut self) -> Socket {
        Socket {
            handle: self.handle.unwrap()
        }
    }
}


pub struct Socket {
    handle: SocketHandle
}
impl Socket {
    fn new(interf: &mut RawsockInterface) {

    }
}
