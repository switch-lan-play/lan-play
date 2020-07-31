use crate::future_smoltcp::{TcpListener, TcpSocket};
use crate::proxy::{BoxProxy, new_tcp_timeout};
use crate::rt::{copy, split, Instant, prelude::*};
use super::timeout_stream::TimeoutStream;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use futures::{future::try_join, stream::{StreamExt, select_all}};

const TCP_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) struct TcpGateway {
    proxy: Arc<BoxProxy>
}

impl TcpGateway {
    pub fn new(proxy: Arc<BoxProxy>) -> TcpGateway {
        TcpGateway {
            proxy,
        }
    }
    pub async fn process(&self, listener: Vec<TcpListener>) -> io::Result<()> {
        let mut listener = select_all(listener.into_iter().map(|i| i.incoming()));
        loop {
            let tcp = listener.next().await.ok_or(io::ErrorKind::NotFound)?;
            let (local_addr, peer_addr) = (tcp.local_addr(), tcp.peer_addr());
            if let Err(e) = self.on_tcp(tcp).await {
                log::error!("on_tcp {:?}", e);
            }
            log::trace!("new tcp  {:?} -> {:?}", peer_addr, local_addr);
        }
    }
    async fn on_tcp(&self, stcp: TcpSocket) -> io::Result<()> {
        let proxy = self.proxy.clone();

        crate::rt::spawn(async move {
            let (local_addr, peer_addr) = (stcp.local_addr(), stcp.peer_addr());
            let ptcp = match new_tcp_timeout(&proxy, stcp.local_addr()?).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("tcp connect to {:?} err {:?}", stcp.local_addr()?, e);
                    return Err(e);
                },
            };

            let start = Instant::now();
            let ptcp = TimeoutStream::new(
                ptcp,
                TCP_TIMEOUT,
            );
            let r = pipe(stcp, ptcp).await;

            log::trace!("tcp done {:?} -> {:?} {:?} {:?}", peer_addr, local_addr, r, start.elapsed());

            Ok::<(), io::Error>(())
        });
        Ok(())
    }
}

async fn pipe<S1, S2>(s1: S1, s2: S2) -> io::Result<(u64, u64)>
where
    S1: AsyncRead + AsyncWrite,
    S2: AsyncRead + AsyncWrite,
{
    let (mut read_1, mut write_1) = split(s1);
    let (mut read_2, mut write_2) = split(s2);

    try_join(
        async {
            let r = copy(&mut read_1, &mut write_2).await;
            write_2.shutdown().await?;
            r
        },
        async {
            let r = copy(&mut read_2, &mut write_1).await;
            write_1.shutdown().await?;
            r
        },
    ).await
}
