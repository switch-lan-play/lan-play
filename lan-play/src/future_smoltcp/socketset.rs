use smoltcp::{
    socket::{self, SocketHandle, SocketSet as InnerSocketSet},
};

#[derive(Debug, Clone, Copy)]
pub struct BufferSize {
    pub tcp_rx_size: usize,
    pub tcp_tx_size: usize,
}

pub struct SocketSet {
    buffer_size: BufferSize,
    set: InnerSocketSet<'static, 'static, 'static>,
}

impl SocketSet {
    pub fn new(buffer_size: BufferSize) -> SocketSet {
        SocketSet {
            buffer_size,
            set: InnerSocketSet::new(vec![]),
        }
    }
    pub fn as_set_mut(&mut self) -> &mut InnerSocketSet<'static, 'static, 'static> {
        &mut self.set
    }
    pub fn remove(&mut self, handle: SocketHandle) {
        self.set.remove(handle);
    }
    pub fn new_tcp_socket(&mut self) -> SocketHandle {
        let handle = self.set.add(self.alloc_tcp_socket());
        handle
    }
    pub fn new_raw_socket(&mut self) -> SocketHandle {
        let handle = self.set.add(self.alloc_raw_socket());
        handle
    }
    fn alloc_tcp_socket(&self) -> socket::TcpSocket<'static> {
        use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
        let rx_buffer = TcpSocketBuffer::new(vec![0; self.buffer_size.tcp_rx_size]);
        let tx_buffer = TcpSocketBuffer::new(vec![0; self.buffer_size.tcp_tx_size]);
        let mut tcp = TcpSocket::new(rx_buffer, tx_buffer);
        tcp.set_accept_all(true);
        tcp.listen(0).unwrap();
    
        tcp
    }
    fn alloc_raw_socket(&self) -> socket::RawSocket<'static, 'static> {
        use smoltcp::socket::{RawPacketMetadata, RawSocket, RawSocketBuffer};
        use smoltcp::wire::{IpProtocol, IpVersion};
        let rx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 32], vec![0; 8192]);
        let tx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 32], vec![0; 8192]);
        let raw = RawSocket::new(IpVersion::Ipv4, IpProtocol::Udp, rx_buffer, tx_buffer);
    
        raw
    }
    
}
