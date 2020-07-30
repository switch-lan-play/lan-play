use super::{other, BoxProxy, BoxTcp, BoxUdp, Proxy, SocketAddr, io, Socks5Proxy};
use shadowsocks::{run_local, Config, ServerConfig, ServerAddr, ConfigType, Mode, crypto::cipher::CipherType};
use std::str::FromStr;
use tokio::runtime::Runtime;

pub struct ShadowsocksProxy {
    _rt: Runtime,
    inner: BoxProxy,
}

impl ShadowsocksProxy {
    pub fn new(url: &url::Url) -> io::Result<BoxProxy> {
        let rt = Runtime::new().unwrap();

        let bind_addr: SocketAddr = "127.13.37.1:50124".parse().unwrap();

        if url.scheme() != "ss" {
            return Err(other("Wrong scheme"))
        }

        match (url.username(), url.password(), url.host_str(), url.port()) {
            ("", None, Some(content), None) => {
                // all base64
                let decoded = base64::decode_config(content, base64::URL_SAFE).map_err(other)?;
                let decoded_str = std::str::from_utf8(&decoded).map_err(other)?;
                let new_url = format!("ss://{}", decoded_str);

                return Self::new(&url::Url::parse(&new_url).map_err(other)?);
            }
            (method, Some(password), Some(host), Some(port)) => {
                let server = ServerConfig::new(
                    ServerAddr::DomainName(host.to_string(), port),
                    password.to_string(),
                    CipherType::from_str(method).map_err(|_| "Failed to parse method").map_err(other)?,
                    None,
                    None,
                );
                let mut config = Config::new(ConfigType::Socks5Local);
                config.server = vec![server];
                config.local_addr = Some(bind_addr.clone().into());
                config.mode = Mode::TcpAndUdp;

                // log::debug!("shadowsocks config: {:#?}", config);
                rt.enter(|| {
                    tokio::spawn(async {
                        let r = run_local(config).await;
                        log::debug!("shadowsocks {:?}", r);
                    });
                });
            }
            _ => return Err(other("Wrong url"))
        }

        Ok(Box::new(Self {
            _rt: rt,
            inner: Socks5Proxy::new(bind_addr.to_string(), None),
        }))
    }
}

#[async_trait]
impl Proxy for ShadowsocksProxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxTcp> {
        self.inner.new_tcp(addr).await
    }
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxUdp> {
        self.inner.new_udp(addr).await
    }
}
