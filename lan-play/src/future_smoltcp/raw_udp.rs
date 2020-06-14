use smoltcp::{wire::{UdpPacket, UdpRepr, Ipv4Packet, Ipv4Repr, IpEndpoint, IpAddress}, Result};
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

impl OwnedUdp {
    pub fn src(&self) -> SocketAddr {
        endpoint2socketaddr(&self.src)
    }
    pub fn dst(&self) -> SocketAddr {
        endpoint2socketaddr(&self.dst)
    }
}

pub fn endpoint2socketaddr(ep: &IpEndpoint) -> SocketAddr {
    let ip = match ep.addr {
        IpAddress::Ipv4(v4) => v4,
        _ => panic!("not ipv4"),
    };
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
 