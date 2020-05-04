use super::EthernetInterface;
use smoltcp::socket::{SocketHandle, TcpSocket, TcpSocketBuffer};

pub enum SocketState {

}

pub struct TcpListener {
    inner: EthernetInterface,
    handle: SocketHandle,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.inner.remove_socket(self.handle);
    }
}

impl TcpListener {
    pub async fn new(inner: EthernetInterface) -> TcpListener {
        let handle = inner.new_socket(new_tcp_socket()).await;
        TcpListener {
            inner,
            handle,
        }
    }
    pub fn accept(&mut self) {

    }
}

fn new_tcp_socket() -> TcpSocket<'static> {
    let rx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    let tx_buffer = TcpSocketBuffer::new(vec![0; 2048]);
    TcpSocket::new(rx_buffer, tx_buffer)
}
