use smoltcp::{wire::{UdpPacket, UdpRepr, Ipv4Packet, Ipv4Repr, IpEndpoint}, phy::ChecksumCapabilities, Result};

#[derive(Debug)]
pub struct Udp<'a> {
    src: IpEndpoint,
    dst: IpEndpoint,
    data: &'a [u8],
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
 