use smoltcp::{
    socket::{self, SocketHandle, Socket, UdpSocket, UdpSocketBuffer, TcpSocket, TcpSocketBuffer, UdpPacketMetadata},
    time::{Instant, Duration},
    phy::{Device, DeviceCapabilities, RxToken, TxToken},
};
use std::sync::Mutex;

pub struct SocketSet {
    set: socket::SocketSet<'static, 'static, 'static>,
    tcp_listener: Option<SocketHandle>,
    udp_listener: Option<SocketHandle>,
}

impl SocketSet {
    pub fn new() -> SocketSet {
        let mut set = SocketSet {
            set: socket::SocketSet::new(vec![]),
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
    pub fn as_set_mut(&mut self) -> &mut socket::SocketSet<'static, 'static, 'static> {
        &mut self.set
    }
}

fn new_tcp_socket() -> TcpSocket<'static> {
    let rx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    let tx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    let mut tcp = TcpSocket::new(rx_buffer, tx_buffer);
    tcp.set_accept_all(true);

    tcp
}

fn new_udp_socket() -> UdpSocket<'static, 'static> {
    let rx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let mut udp = UdpSocket::new(rx_buffer, tx_buffer);
    udp.set_accept_all(true);

    udp
}
