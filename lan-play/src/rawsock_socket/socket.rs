use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer, SocketSet, SocketHandle},
    wire::{IpCidr, IpAddress}
};
use super::interface::{RawsockInterface};
use async_std::sync::{Arc, RwLock};
use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use super::event::Event;
use super::Result;

#[derive(Debug)]
pub(super) struct AsyncSocket {
    pub handle: SocketHandle,
    pub can_read: mpsc::Receiver<()>,
    pub can_write: mpsc::Receiver<()>,
}

pub struct TcpListener<'a> {
    handle: AsyncSocket,
    interf: &'a RawsockInterface,
}

impl<'a> TcpListener<'a> {
    pub async fn new(interf: &'a RawsockInterface) -> Result<TcpListener<'a>> {
        let (s, r) = oneshot::channel::<AsyncSocket>();
        interf.send_event(Event::NewSocket(s)).await?;
        let s = r.await?;
        Ok(TcpListener {
            handle: s,
            interf
        })
    }
    pub async fn next(&mut self) -> Result<Option<Socket>> {
        self.handle.can_read.next().await;
        let next = Self::new(self.interf).await?;
        let handle = std::mem::replace(&mut self.handle, next.handle);
        Ok(Some(Socket {
            handle
        }))
    }
}


pub struct Socket {
    handle: AsyncSocket
}
impl Socket {
    fn new(interf: &mut RawsockInterface) {

    }
}
