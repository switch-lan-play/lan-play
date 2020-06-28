use smoltcp::{
    socket::{self, SocketHandle, SocketSet as InnerSocketSet},
};

pub struct SocketSet {
    set: InnerSocketSet<'static, 'static, 'static>,
}

impl SocketSet {
    pub fn new() -> SocketSet {
        SocketSet {
            set: InnerSocketSet::new(vec![]),
        }
    }
    pub fn as_set_mut(&mut self) -> &mut InnerSocketSet<'static, 'static, 'static> {
        &mut self.set
    }
    pub fn new_tcp_socket(&mut self) -> SocketHandle {
        let handle = self.set.add(new_tcp_socket());
        handle
    }
    pub fn new_raw_socket(&mut self) -> SocketHandle {
        let handle = self.set.add(new_raw_socket());
        handle
    }
}

fn new_tcp_socket() -> socket::TcpSocket<'static> {
    use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
    let rx_buffer = TcpSocketBuffer::new(vec![0; 65536]);
    let tx_buffer = TcpSocketBuffer::new(vec![0; 65536]);
    let mut tcp = TcpSocket::new(rx_buffer, tx_buffer);
    tcp.set_accept_all(true);
    tcp.listen(0).unwrap();

    tcp
}

fn new_raw_socket() -> socket::RawSocket<'static, 'static> {
    use smoltcp::socket::{RawPacketMetadata, RawSocket, RawSocketBuffer};
    use smoltcp::wire::{IpProtocol, IpVersion};
    let rx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 32], vec![0; 8192]);
    let tx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 32], vec![0; 8192]);
    let raw = RawSocket::new(IpVersion::Ipv4, IpProtocol::Udp, rx_buffer, tx_buffer);

    raw
}
