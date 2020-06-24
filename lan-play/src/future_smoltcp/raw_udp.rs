use smoltcp::{wire::{UdpPacket, UdpRepr, Ipv4Packet, Ipv4Repr, IpEndpoint, IpAddress, Ipv4Address, IpProtocol}, Result};
pub use smoltcp::phy::ChecksumCapabilities;
use std::net::{SocketAddr, SocketAddrV4};

#[derive(Debug)]
pub struct Udp<'a> {
    pub src: IpEndpoint,
    pub dst: IpEndpoint,
    pub data: &'a [u8],
}

#[derive(Debug)]
pub struct OwnedUdp {
    pub src: IpEndpoint,
    pub dst: IpEndpoint,
    pub data: Vec<u8>,
}

trait UnwrapV4 {
    fn unwrap_v4(self) -> Ipv4Address;
}

impl UnwrapV4 for IpAddress {
    fn unwrap_v4(self) -> Ipv4Address {
        match self {
            IpAddress::Ipv4(v4) => v4,
            _ => panic!("not v4"),
        }
    }
}

impl OwnedUdp {
    pub fn new(src: SocketAddr, dst: SocketAddr, data: Vec<u8>) -> OwnedUdp {
        OwnedUdp {
            src: src.into(),
            dst: dst.into(),
            data,
        }
    }
    pub fn src(&self) -> SocketAddr {
        endpoint2socketaddr(&self.src)
    }
    pub fn dst(&self) -> SocketAddr {
        endpoint2socketaddr(&self.dst)
    }
    pub fn to_raw(&self) -> Vec<u8> {
        let checksum = ChecksumCapabilities::default();
        let udp_repr = UdpRepr {
            src_port: self.src.port,
            dst_port: self.dst.port,
            payload: &self.data,
        };
        let src_addr = self.src.addr;
        let dst_addr = self.dst.addr;
        let ip_repr = Ipv4Repr {
            src_addr: src_addr.unwrap_v4(),
            dst_addr: dst_addr.unwrap_v4(),
            protocol: IpProtocol::Udp,
            payload_len: udp_repr.buffer_len(),
            hop_limit: 64,
        };
        let mut bytes = vec![0xa5; ip_repr.buffer_len() + udp_repr.buffer_len()];
        let mut udp_packet = UdpPacket::new_unchecked(&mut bytes[ip_repr.buffer_len()..]);
        udp_repr.emit(&mut udp_packet, &src_addr, &dst_addr, &checksum);
        let mut ip_packet = Ipv4Packet::new_unchecked(&mut bytes);
        ip_repr.emit(&mut ip_packet, &checksum);
        bytes
    }
}

pub fn endpoint2socketaddr(ep: &IpEndpoint) -> SocketAddr {
    let ip = ep.addr.unwrap_v4();
    SocketAddr::V4(SocketAddrV4::new(
        ip.0.into(),
        ep.port,
    ))
}

pub fn parse_udp_owned(data: &[u8], checksum_caps: &ChecksumCapabilities) -> Result<OwnedUdp> {
    let Udp { src, dst, data } = parse_udp(data, checksum_caps)?;
    Ok(OwnedUdp {
        src,
        dst,
        data: data.to_owned(),
    })
}

pub fn parse_udp<'a>(data: &'a [u8], checksum_caps: &ChecksumCapabilities) -> Result<Udp<'a>> {
    let ipv4_packet = Ipv4Packet::new_checked(data)?;
    let ipv4_repr = Ipv4Repr::parse(&ipv4_packet, &checksum_caps)?;
    let udp_packet = UdpPacket::new_checked(ipv4_packet.payload())?;
    let udp_repr = UdpRepr::parse(
        &udp_packet,
        &ipv4_repr.src_addr.into(), &ipv4_repr.dst_addr.into(),
        checksum_caps
    )?;
    let src = IpEndpoint {
        addr: ipv4_repr.src_addr.into(),
        port: udp_repr.src_port,
    };
    let dst = IpEndpoint {
        addr: ipv4_repr.dst_addr.into(),
        port: udp_repr.dst_port,
    };
    let data = udp_repr.payload;
    Ok(Udp {
        src,
        dst,
        data
    })
}
 