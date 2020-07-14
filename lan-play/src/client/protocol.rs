#![allow(dead_code)]

use thiserror::Error;

mod forwarder_type {
    pub const KEEPALIVE: u8 = 0;
    pub const IPV4: u8 = 1;
    pub const PING: u8 = 2;
    pub const IPV4_FRAG: u8 = 3;
    pub const AUTH_ME: u8 = 4;
    pub const INFO: u8 = 0x10;
}
mod field {
    pub type Field = ::core::ops::Range<usize>;
    pub type FieldFrom = ::core::ops::RangeFrom<usize>;
    pub const SRC_IP: Field = 12..16;
    pub const DST_IP: Field = 16..20;
    pub const FRAG_SRC_IP: Field = 0..4;
    pub const FRAG_DST_IP: Field = 4..8;
    pub const FRAG_ID: Field = 8..10;
    pub const FRAG_PART: usize = 10;
    pub const FRAG_TOTAL_PART: usize = 11;
    pub const FRAG_LEN: Field = 12..14;
    pub const FRAG_PMTU: Field = 14..16;
    pub const FRAG_DATA: FieldFrom = 16..;
}

#[derive(Debug, Clone, Copy, Error)]
pub enum ParseError {
    #[error("the data is not parseable")]
    NotParseable,
}
pub type Result<T> = std::result::Result<T, ParseError>;

pub trait Parser<'a> {
    const MIN_LENGTH: usize;
    const MAX_LENGTH: usize = 2048;

    fn do_parse(bytes: &'a [u8]) -> Result<Self>
    where
        Self: Sized;

    fn parse(bytes: &'a [u8]) -> Result<Self>
    where
        Self: Sized,
    {
        if bytes.len() < Self::MIN_LENGTH || bytes.len() > Self::MAX_LENGTH {
            Err(ParseError::NotParseable)
        } else {
            Self::do_parse(bytes)
        }
    }
}

pub trait Builder {
    fn build(self) -> Vec<u8>;
}

#[derive(Debug)]
pub enum ForwarderFrame<'a> {
    Keepalive,
    Ipv4(Ipv4<'a>),
    Ping(Ping<'a>),
    Ipv4Frag(Ipv4Frag<'a>),
    AuthMe,
    Info,
}

impl<'a> Parser<'a> for ForwarderFrame<'a> {
    const MIN_LENGTH: usize = 1;
    fn do_parse(bytes: &'a [u8]) -> Result<ForwarderFrame> {
        let typ = bytes[0];
        let rest = &bytes[1..];
        let frame = match typ {
            forwarder_type::KEEPALIVE => ForwarderFrame::Keepalive,
            forwarder_type::IPV4 => ForwarderFrame::Ipv4(Ipv4::parse(&rest)?),
            forwarder_type::PING => ForwarderFrame::Ping(Ping::parse(&rest)?),
            forwarder_type::IPV4_FRAG => ForwarderFrame::Ipv4Frag(Ipv4Frag::parse(&rest)?),
            forwarder_type::AUTH_ME => ForwarderFrame::AuthMe,
            forwarder_type::INFO => ForwarderFrame::Info,
            _ => return Err(ParseError::NotParseable),
        };
        Ok(frame)
    }
}

impl<'a> Builder for ForwarderFrame<'a> {
    fn build(self) -> Vec<u8> {
        let r = match self {
            ForwarderFrame::Keepalive => [forwarder_type::KEEPALIVE],
            ForwarderFrame::Ipv4(_ipv4) => [forwarder_type::IPV4],
            ForwarderFrame::Ping(_ping) => [forwarder_type::PING],
            ForwarderFrame::Ipv4Frag(_ipv4) => [forwarder_type::IPV4_FRAG],
            ForwarderFrame::AuthMe => [forwarder_type::AUTH_ME],
            ForwarderFrame::Info => [forwarder_type::INFO],
        };
        Vec::from(r)
    }
}

#[derive(Debug)]
pub struct Ipv4<'a> {
    payload: &'a [u8],
}

impl<'a> Ipv4<'a> {
    pub fn new(payload: &'a [u8]) -> Ipv4<'a> {
        Ipv4 {
            payload
        }
    }
    pub fn payload(&self) -> &[u8] {
        self.payload
    }
}

impl<'a> Parser<'a> for Ipv4<'a> {
    const MIN_LENGTH: usize = 20;
    fn do_parse(bytes: &'a [u8]) -> Result<Ipv4> {
        Ok(Ipv4 { payload: bytes })
    }
}

#[derive(Debug)]
pub struct Ping<'a> {
    payload: &'a [u8],
}

impl<'a> Parser<'a> for Ping<'a> {
    const MIN_LENGTH: usize = 4;
    const MAX_LENGTH: usize = 4;
    fn do_parse(bytes: &'a [u8]) -> Result<Ping> {
        Ok(Ping { payload: bytes })
    }
}

#[derive(Debug)]
pub struct Ipv4Frag<'a> {
    payload: &'a [u8],
}

impl<'a> Parser<'a> for Ipv4Frag<'a> {
    const MIN_LENGTH: usize = 16;
    fn do_parse(bytes: &'a [u8]) -> Result<Ipv4Frag> {
        Ok(Ipv4Frag { payload: bytes })
    }
}