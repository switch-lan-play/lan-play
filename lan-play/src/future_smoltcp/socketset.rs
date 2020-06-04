use smoltcp::{
    socket::{self, SocketHandle, SocketSet as InnerSocketSet},
};
use super::socket::{TcpSocket};

pub struct SocketSet {
    set: InnerSocketSet<'static, 'static, 'static>,
    tcp_listener: Option<SocketHandle>,
    udp_listener: Option<SocketHandle>,
}

impl SocketSet {
    pub fn new() -> SocketSet {
        let mut set = SocketSet {
            set: InnerSocketSet::new(vec![]),
            tcp_listener: None,
            udp_listener: None,
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
    pub fn get_new_tcp(&mut self) -> Option<TcpSocket> {
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
