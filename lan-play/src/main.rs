#![recursion_limit="256"]

#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate async_trait;

mod client;
mod error;
mod future_smoltcp;
mod gateway;
mod interface_info;
mod lan_play;
mod proxy;
mod interface;
mod rt;

use client::LanClient;
use error::Result;
use lan_play::LanPlay;
use proxy::{DirectProxy, Auth, BoxProxy};
use rawsock::traits::Library;
use interface::RawsockInterfaceSet;
use smoltcp::wire::Ipv4Cidr;
use std::net::Ipv4Addr;
use url::Url;

#[cfg(feature = "logging-allocator")]
#[global_allocator]
static ALLOC: logging_allocator::LoggingAllocator = logging_allocator::LoggingAllocator::new();

#[cfg(feature = "socks5")]
use proxy::Socks5Proxy;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
enum Subcommand {
    /// Check proxy setting
    Check {
        /// Proxy setting e.g. socks5://localhost:1080
        #[structopt(short, long, parse(try_from_str = Url::parse))]
        proxy: Option<Url>,
    },
    /// Send a ping to relay server
    Ping {
        /// Relay server e.g. localhost:11451
        #[structopt()]
        relay: String,
        #[structopt(short)]
        times: Option<u64>,
    },
}

/// Lan play
#[derive(Debug, StructOpt)]
struct Opt {
    /// IP Address
    #[structopt(short, long, parse(try_from_str = str::parse), default_value = "10.13.37.2")]
    gateway_ip: Ipv4Addr,

    /// MTU
    #[structopt(long, default_value = "1400")]
    mtu: usize,

    /// Prefix length
    #[structopt(long, default_value = "16")]
    prefix_len: u8,

    /// Network interface
    #[structopt(short = "i", long, env = "LP_NETIF")]
    netif: Option<String>,

    /// Proxy setting e.g. socks5://localhost:1080
    #[structopt(short, long, parse(try_from_str = Url::parse), env = "LP_PROXY")]
    proxy: Option<Url>,

    /// Relay server e.g. localhost:11451
    #[structopt(short, long, env = "LP_RELAY")]
    relay: Option<String>,

    /// Optional subcommand
    #[structopt(subcommand)]
    subcommand: Option<Subcommand>,
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

fn parse_proxy(proxy: &Option<Url>) -> BoxProxy {
    let r = match proxy {
        #[cfg(feature = "socks5")]
        Some(url) if url.scheme() == "socks5" => {
            let (addr, auth) = url_into_addr_auth(&url).expect("Failed to parse proxy url");
            log::info!("Use socks5 proxy: {}", url);
            Some(Socks5Proxy::new(addr, auth))
        },
        #[cfg(feature = "shadowsocks")]
        Some(url) if url.scheme() == "ss" => {
            log::info!("Use shadowsocks proxy: {}", url);
            proxy::ShadowsocksProxy::new(url).ok()
        },
        None => {
            Some(DirectProxy::new())
        },
        Some(url) => {
            log::warn!("Unrecognized proxy url: {}, not using proxy", url);
            Some(DirectProxy::new())
        }
    };
    match r {
        Some(p) => p,
        None => {
            log::warn!("Failed to parse proxy url: {:?}", proxy);
            DirectProxy::new()
        }
    }
}

async fn run(opt: Opt) -> Result<()> {
    let ipv4cidr = Ipv4Cidr::new(opt.gateway_ip.into(), opt.prefix_len);
    let gateway_ip = opt.gateway_ip.into();
    let proxy = parse_proxy(&opt.proxy);
    let client = match opt.relay {
        Some(relay) => Some(LanClient::new(relay, ipv4cidr).await?),
        None => None,
    };

    let set = RawsockInterfaceSet::new(&RAWSOCK_LIB, ipv4cidr)
        .expect("Could not open any packet capturing library");

    let mut lp = LanPlay::new(proxy, ipv4cidr, gateway_ip, opt.mtu);

    lp.start(&set, opt.netif, client).await?;

    Ok(())
}

async fn ping(relay: &str, times: &Option<u64>) -> Result<()> {
    use crate::rt::{Instant, Duration, timeout, delay_for};

    let client = LanClient::new(relay.to_string(), Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED.into(), 0)).await?;
    let times = times.unwrap_or(4);

    for i in 0..times {
        let start = Instant::now();
        timeout(Duration::from_secs(1), client.ping()).await??;
        println!("ping responsed in {:?} (#{})", start.elapsed(), i + 1);
        if i + 1 != times {
            delay_for(Duration::from_secs(1)).await;
        }
    }

    Ok(())
}

async fn check(proxy: &Option<Url>) -> Result<()> {
    use crate::rt::prelude::*;

    let domain = "example.org";
    let proxy = parse_proxy(proxy);

    println!("querying DNS record of {}", domain);
    let addr = proxy::resolve(
        &proxy,
        "8.8.8.8:53".parse().unwrap(),
        domain,
    ).await?;
    println!("UDP test passed");

    let addr = addr.first();

    if let Some(addr) = addr {
        let success = "HTTP/1.0 200 OK\r\n";
        let addr = *addr;
        println!("connecting to {}({:?})", domain, addr);
        let mut tcp = proxy.new_tcp(std::net::SocketAddr::new(addr.into(), 80)).await?;
        println!("connected");
        let req = format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", domain);
        tcp.write_all(req.as_bytes()).await?;
        let mut ret = String::new();
        tcp.read_to_string(&mut ret).await?;
        if &ret[..success.len()] == success {
            println!("TCP test passed");
        } else {
            println!("TCP test failed. Response: {}", ret);
        }
    }

    Ok(())
}

#[rt::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let opt = Opt::from_args();
    #[cfg(feature = "logging-allocator")]
    ALLOC.enable_logging();
    match &opt.subcommand {
        Some(Subcommand::Ping { relay, times  }) => ping(relay, times).await,
        Some(Subcommand::Check { proxy }) => check(proxy).await,
        None => run(opt).await,
    }
}
