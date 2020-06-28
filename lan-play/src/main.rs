#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate async_trait;

mod error;
mod future_smoltcp;
mod gateway;
mod interface_info;
mod lan_play;
mod proxy;
mod rawsock_socket;

use error::Result;
use lan_play::LanPlay;
use proxy::{DirectProxy, Socks5Proxy};
use rawsock::traits::Library;
use rawsock_socket::RawsockInterfaceSet;
use smoltcp::wire::Ipv4Cidr;
use std::net::Ipv4Addr;

use structopt::StructOpt;

/// Lan play
#[derive(Debug, StructOpt)]
struct Opt {
    /// IP Address
    #[structopt(short, long, parse(try_from_str = str::parse), default_value = "10.13.37.2")]
    gateway_ip: Ipv4Addr,
    /// Prefix length
    #[structopt(short, long, default_value = "16")]
    prefix_len: u8,
    /// Network interface
    #[structopt(short = "i", long, env = "LP_NETIF")]
    netif: Option<String>,
}

lazy_static! {
    static ref RAWSOCK_LIB: Box<dyn Library> = {
        let lib = open_best_library().expect("Can't open any library");
        println!("Library opened, version is {}", lib.version());
        lib
    };
}

fn open_best_library() -> Result<Box<dyn Library>> {
    if let Ok(l) = rawsock::wpcap::Library::open_default_paths() {
        return Ok(Box::new(l));
    }
    Ok(Box::new(rawsock::pcap::Library::open_default_paths()?))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let opt = Opt::from_args();
    let ipv4cidr = Ipv4Cidr::new(opt.gateway_ip.into(), opt.prefix_len);
    let gateway_ip = opt.gateway_ip.into();

    let set = RawsockInterfaceSet::new(&RAWSOCK_LIB, ipv4cidr)
        .expect("Could not open any packet capturing library");

    let _direct = DirectProxy::new();
    let _socks5 = Socks5Proxy::new("127.0.0.1:10800".parse().unwrap(), None);
    let mut lp = LanPlay::new(_direct, ipv4cidr, gateway_ip);

    lp.start(&set, opt.netif).await?;

    Ok(())
}
