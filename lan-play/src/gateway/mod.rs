mod timeout_stream;
mod tcp;
mod udp;

use crate::future_smoltcp::{TcpListener, UdpSocket};
use crate::proxy::BoxProxy;
use std::io;
use std::sync::Arc;
use futures::try_join;
use tcp::TcpGateway;
use udp::UdpGateway;

pub struct Gateway {
    tcp: TcpGateway,
    udp: UdpGateway,
}

impl Gateway {
    pub fn new(proxy: BoxProxy) -> Gateway {
        let proxy = Arc::new(proxy);
        Gateway {
            tcp: TcpGateway::new(proxy.clone()),
            udp: UdpGateway::new(proxy.clone()),
        }
    }
    pub async fn process(&self, tcp: Vec<TcpListener>, udp: UdpSocket) -> io::Result<()> {
        try_join!(self.tcp.process(tcp), self.udp.process(udp))?;
        Ok(())
    }
}
