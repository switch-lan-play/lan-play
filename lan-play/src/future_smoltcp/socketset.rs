use smoltcp::{
    socket::{self, SocketHandle, SocketSet as InnerSocketSet, AnySocket}, phy::ChecksumCapabilities,
};
use super::raw_udp::parse_udp;
use super::{OutPacket, socket::{TcpSocket, UdpSocket, Socket, SocketLeaf}};
use tokio::sync::mpsc;
use std::collections::HashMap;

pub struct SocketSet {
    set: InnerSocketSet<'static, 'static, 'static>,
    tcp_listener: Option<SocketHandle>,
    raw_socket: SocketHandle,
    socket_sender: mpsc::Sender<Socket>,
    packet_sender: mpsc::Sender<OutPacket>,
    leaf_map: HashMap<SocketHandle, SocketLeaf>,
}

impl SocketSet {
    pub fn new(socket_sender: mpsc::Sender<Socket>, packet_sender: mpsc::Sender<OutPacket>) -> SocketSet {
        let mut nset = InnerSocketSet::new(vec![]);
        let raw_socket = nset.add(new_raw_socket());
        let mut set = SocketSet {
            set: nset,
            tcp_listener: None,
            raw_socket,
            socket_sender,
            packet_sender,
            leaf_map: HashMap::new(),
        };
        set.preserve_socket();
        set
    }
    fn preserve_socket(&mut self) {
        if self.tcp_listener.is_none() {
            let handle = self.set.add(new_tcp_socket());
            self.tcp_listener = Some(handle)
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
            let (socket, leaf) = TcpSocket::new(handle, self.packet_sender.clone());
            self.leaf_map.insert(handle, leaf);
            Some(socket)
        }
    }
    pub async fn send(&mut self, handle: SocketHandle, data: Vec<u8>) {
        println!("TODO: socketset.send");
        let socket = self.set.get::<socket::TcpSocket>(handle);
    }
    pub async fn process(&mut self)  {
        if let Some(tcp) = self.get_new_tcp() {
            self.socket_sender.send(tcp.into()).await.unwrap();
        }
        {
            let mut raw = self.set.get::<socket::RawSocket>(self.raw_socket);
            if raw.can_recv() {
                let data = raw.recv().unwrap();
                let udp = parse_udp(data, &ChecksumCapabilities::default());
                println!("udp {:?}", udp);
            }
        }
        for mut s in self.set.iter_mut().filter_map(socket::TcpSocket::downcast) {
            if s.may_recv() {
                println!("may_recv {:?}", s.handle());
                let data = s.recv(|buffer| {
                    let data = buffer.to_owned();
                    (data.len(), data)
                }).unwrap();
                println!("data {:?}", data);
                if let Some(leaf) = self.leaf_map.get_mut(&s.handle()) {
                    leaf.send(data).await;
                } else {
                    println!("no leaf");
                }
                
            }
            // println!("tcp {:?} recv {} send {}", s.state(), s.may_recv(), s.may_send());
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

fn new_raw_socket() -> socket::RawSocket<'static, 'static> {
    use smoltcp::socket::{RawSocket, RawSocketBuffer, RawPacketMetadata};
    use smoltcp::wire::{IpVersion, IpProtocol};
    let rx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let tx_buffer = RawSocketBuffer::new(vec![RawPacketMetadata::EMPTY; 4], vec![0; 2048]);
    let raw = RawSocket::new(
        IpVersion::Ipv4, IpProtocol::Udp,
        rx_buffer, tx_buffer
    );

    raw
}
