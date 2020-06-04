use smoltcp::{
    socket::{self, SocketHandle, SocketSet as InnerSocketSet, AnySocket},
};
use super::socket::{TcpSocket, Socket};
use tokio::sync::mpsc;

pub struct SocketSet {
    set: InnerSocketSet<'static, 'static, 'static>,
    tcp_listener: Option<SocketHandle>,
    udp_listener: Option<SocketHandle>,
    socket_sender: mpsc::Sender<Socket>,
}

impl SocketSet {
    pub fn new(socket_sender: mpsc::Sender<Socket>) -> SocketSet {
        let mut set = SocketSet {
            set: InnerSocketSet::new(vec![]),
            tcp_listener: None,
            udp_listener: None,
            socket_sender,
        };
        set.preserve_socket();
        set
    }
    fn preserve_socket(&mut self) {
        if self.tcp_listener.is_none() {
            let handle = self.set.add(new_tcp_socket());
            self.tcp_listener = Some(handle)
        }
        if self.udp_listener.is_none() {
            let handle = self.set.add(new_udp_socket());
            self.udp_listener = Some(handle)
        }
    }
    pub fn as_set_mut(&mut self) -> &mut InnerSocketSet<'static, 'static, 'static> {
        &mut self.set
    }
    fn get_new_tcp(&mut self) -> Option<TcpSocket> {
        let handle = match self.tcp_listener {
            Some(handle) => handle,
            None => return None,
        };
        let listening = self.set.get::<socket::TcpSocket>(handle).is_listening();
        if listening {
            None
        } else {
            self.tcp_listener.take();
            self.preserve_socket();
            Some(TcpSocket::new(handle))
        }
    }
    pub async fn process(&mut self) {
        if let Some(tcp) = self.get_new_tcp() {
            self.socket_sender.send(tcp.into()).await.unwrap();
        }
        for s in self.set.iter_mut().filter_map(socket::TcpSocket::downcast) {
            println!("tcp {:?} recv {} send {}", s.state(), s.may_recv(), s.may_send());
        }
    }
}

fn new_tcp_socket() -> socket::TcpSocket<'static> {
    use smoltcp::socket::{TcpSocketBuffer, TcpSocket};
    let rx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    let tx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    let mut tcp = TcpSocket::new(rx_buffer, tx_buffer);
    tcp.set_accept_all(true);
    tcp.listen(0).unwrap();

    tcp
}

fn new_udp_socket() -> socket::UdpSocket<'static, 'static> {
    use smoltcp::socket::{UdpSocket, UdpSocketBuffer, UdpPacketMetadata};
    let rx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let mut udp = UdpSocket::new(rx_buffer, tx_buffer);
    udp.set_accept_all(true);

    udp
}
