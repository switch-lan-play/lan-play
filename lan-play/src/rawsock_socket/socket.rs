use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer, SocketSet, SocketHandle},
    wire::{IpCidr, IpAddress}
};
use super::interface::{RawsockInterface, SharedSockets};
use async_std::sync::{Arc, RwLock};

pub struct TcpListener {
    handle: SocketHandle
}

impl<'a> TcpListener {
    pub async fn new(interf: &RawsockInterface) -> TcpListener {
        let handle = interf.new_socket().await;
        interf.borrow_socket::<TcpSocket, _, _>(handle, |socket| {
            socket.set_accept_all(true);
        }).await;
        TcpListener {
            handle
        }
    }
    pub async fn next(&mut self) -> Socket {
        Socket {
            handle: self.handle
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
