use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{EthernetAddress, Ipv4Cidr},
    socket::{SocketSet, SocketHandle, AnySocket, TcpSocketBuffer, TcpSocket},
};
use super::device::{FutureDevice, Packet, AsyncIface};
use futures::channel::mpsc::{channel, Sender, Receiver};
use futures::channel::oneshot;
use futures::prelude::*;
use futures::select;

type DeviceType = FutureDevice<Receiver<Packet>, Sender<Packet>>;
type IFace = EthernetInterface<'static, 'static, 'static, DeviceType>;
pub type Sockets = SocketSet<'static, 'static, 'static>;
pub type Callback = Box<dyn Fn(&mut Sockets) + Send>;
type CallbackWrapper = (Callback, oneshot::Sender<()>);

/// A wrapper struct for EthernetInterface and it's sockets
pub struct Net {
    sockets: Sockets,
    iface: IFace,
    waiting: Vec<SocketHandle>,
    backlog: usize,
    receiver: Receiver<CallbackWrapper>,
    sender: Sender<CallbackWrapper>,
}

impl Net {
    fn new_tcp_socket(sockets: &mut Sockets) -> SocketHandle {
        let rx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
        let tx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
        let socket = TcpSocket::new(rx_buffer, tx_buffer);
        sockets.add(socket)
    }
    pub fn new(iface: IFace, backlog: usize) -> Self {
        let mut waiting: Vec<SocketHandle> = vec![];
        let (sender, receiver) = channel(1);
        let mut sockets = SocketSet::new(vec![]);

        for _ in 0..backlog {
            waiting.push(Self::new_tcp_socket(&mut sockets))
        }
        Self {
            sockets,
            iface,
            waiting: vec![],
            backlog,
            receiver,
            sender,
        }
    }
    pub async fn wait(&mut self) -> super::Result<()> {
        select!{
            _ = self.iface.poll_async(&mut self.sockets).fuse() => (),
            item = self.receiver.next().fuse() => {
                if let Some((cb, next)) = item {
                    cb(&mut self.sockets);
                    next.send(()).unwrap();
                }
            }
        }

        Ok(())
    }
    pub fn borrow_sockets(&mut self) -> &mut Sockets {
        &mut self.sockets
    }
    pub async fn send(&self, item: Callback) -> super::Result<()> {
        let (s, r) = oneshot::channel();
        self.sender.clone().send((item, s)).await?;
        r.await?;
        Ok(())
    }
}
