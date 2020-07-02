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
use proxy::{DirectProxy, Socks5Proxy, Auth};
use rawsock::traits::Library;
use rawsock_socket::RawsockInterfaceSet;
use smoltcp::wire::Ipv4Cidr;
use std::net::Ipv4Addr;
use url::Url;

use structopt::StructOpt;

/// Lan play
#[derive(Debug, StructOpt)]
struct Opt {
    /// IP Address
    #[structopt(short, long, parse(try_from_str = str::parse), default_value = "10.13.37.2")]
    gateway_ip: Ipv4Addr,
    /// Prefix length
    #[structopt(long, default_value = "16")]
    prefix_len: u8,
    /// Network interface
    #[structopt(short = "i", long, env = "LP_NETIF")]
    netif: Option<String>,
    /// Proxy setting e.g. socks5://localhost:1080
    #[structopt(short, long, parse(try_from_str = Url::parse), env = "LP_PROXY")]
    proxy: Option<Url>,
}

lazy_static! {
    static ref RAWSOCK_LIB: Box<dyn Library> = {
        let lib = open_best_library().expect("Can't open any library");
        log::info!("Library opened, version is {}", lib.version());
        lib
    };
}

fn open_best_library() -> Result<Box<dyn Library>> {
    if let Ok(l) = rawsock::wpcap::Library::open_default_paths() {
        return Ok(Box::new(l));
    }
    Ok(Box::new(rawsock::pcap::Library::open_default_paths()?))
}

fn url_into_addr_auth(url: &Url) -> Option<(String, Option<Auth>)> {
    let auth: Option<Auth> = match (url.has_authority(), url.username(), url.password()) {
        (true, username, password) => Some(
            Auth {
                username: username.to_string(),
                password: password.unwrap_or("").to_string(),
            }
        ),
        _ => None,
    };
    match (url.scheme(), url.host_str(), url.port_or_known_default()) {
        ("socks5", Some(host), Some(port)) => {
            Some((format!("{}:{}", host, port), auth))
        }
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let opt = Opt::from_args();
    let ipv4cidr = Ipv4Cidr::new(opt.gateway_ip.into(), opt.prefix_len);
    let gateway_ip = opt.gateway_ip.into();
    let proxy = match opt.proxy {
        Some(url) if url.scheme() == "socks5" => {
            let (addr, auth) = url_into_addr_auth(&url).expect("Failed to parse proxy url");
            log::info!("Use socks5 proxy: {}", url);
            Socks5Proxy::new(addr, auth)
        },
        None => {
            DirectProxy::new()
        },
        Some(url) => {
            log::warn!("Unrecognized proxy url: {}, not using proxy", url);
            DirectProxy::new()
        }
    };

    let set = RawsockInterfaceSet::new(&RAWSOCK_LIB, ipv4cidr)
        .expect("Could not open any packet capturing library");

    let mut lp = LanPlay::new(proxy, ipv4cidr, gateway_ip);

    lp.start(&set, opt.netif).await?;

    Ok(())
}
