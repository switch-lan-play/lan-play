use tokio::{sync::Notify, time::delay_for};
use super::{Ethernet, SocketHandle, SocketSet, BufferSize};
use futures::prelude::*;
use futures::select;
use futures::future::poll_fn;
use smoltcp::{socket::TcpState, time::{Duration, Instant}};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Poll, Waker};

#[derive(Debug)]
pub struct Wakers {
    readers: Vec<Waker>,
    writers: Vec<Waker>,
}

pub(super) struct Source {
    wakers: Mutex<Wakers>,
}

pub(super) struct NetReactor {
    socket_set: Mutex<SocketSet>,
    sources: Mutex<HashMap<SocketHandle, Arc<Source>>>,
    notify: Notify,
}

impl Source {
    pub async fn readable(&self, _reactor: &NetReactor) -> io::Result<()> {
        let mut polled = false;

        future::poll_fn(|cx| {
            if polled {
                Poll::Ready(Ok(()))
            } else {
                let mut wakers = self.wakers.lock().unwrap();

                if wakers.readers.iter().all(|w| !w.will_wake(cx.waker())) {
                    wakers.readers.push(cx.waker().clone());
                }

                polled = true;
                Poll::Pending
            }
        })
        .await
    }
    pub async fn writable(&self, _reactor: &NetReactor) -> io::Result<()> {
        let mut polled = false;

        future::poll_fn(|cx| {
            if polled {
                Poll::Ready(Ok(()))
            } else {
                let mut wakers = self.wakers.lock().unwrap();

                if wakers.writers.iter().all(|w| !w.will_wake(cx.waker())) {
                    wakers.writers.push(cx.waker().clone());
                }

                polled = true;
                Poll::Pending
            }
        })
        .await
    }
}

impl NetReactor {
    pub fn new(buffer_size: BufferSize) -> Arc<NetReactor> {
        Arc::new(NetReactor {
            socket_set: Mutex::new(SocketSet::new(buffer_size)),
            sources: Mutex::new(HashMap::new()),
            notify: Notify::new(),
        })
    }
    pub fn lock_set(&self) -> MutexGuard<'_, SocketSet> {
        self.socket_set.lock().unwrap()
    }
    pub fn insert(&self, handle: SocketHandle) -> Arc<Source> {
        let source = Arc::new(Source {
            wakers: Mutex::new(Wakers {
                readers: Vec::new(),
                writers: Vec::new(),
            }),
        });
        self.sources.lock().unwrap().insert(handle, source.clone());
        source
    }
    pub fn remove(&self, handle: &SocketHandle) {
        self.sources.lock().unwrap().remove(handle);
    }
    pub fn notify(&self) {
        self.notify.notify();
    }
    pub async fn run(&self, mut ethernet: Ethernet) {
        let default_timeout = Duration::from_secs(10);
        let sockets = &self.socket_set;
        let mut ready = Vec::new();

        loop {
            let start = Instant::now();
            let deadline = {
                ethernet
                    .poll_delay(sockets.lock().unwrap().as_set_mut(), start)
                    .unwrap_or(default_timeout)
            };
            let device = ethernet.device_mut();

            if device.need_wait() {
                select! {
                    _ = delay_for(deadline.into()).fuse() => {},
                    _ = device.wait().fuse() => {},
                    _ = self.notify.notified().fuse() => {},
                }
            }
            let mut set = sockets.lock().unwrap();
            let end = Instant::now();
            match ethernet.poll(set.as_set_mut(), end) {
                Ok(true) => (),
                // readiness not changed
                Ok(false) | Err(smoltcp::Error::Dropped) => continue,
                Err(e) => {
                    log::error!("poll error {:?}", e);
                    continue;
                }
            };

            let sources = self.sources.lock().unwrap();
            for socket in set.as_set_mut().iter() {
                let (readable, writable) = match socket {
                    smoltcp::socket::Socket::Tcp(tcp) => (
                        tcp.can_recv() || is_going_to_close(tcp.state()),
                        tcp.can_send() || is_going_to_close(tcp.state()),
                    ),
                    smoltcp::socket::Socket::Raw(raw) => (raw.can_recv(), raw.can_send()),
                    _ => continue, // ignore other type
                };
                let handle = socket.handle();

                if let Some(source) = sources.get(&handle) {
                    let mut wakers = source.wakers.lock().unwrap();

                    if readable {
                        ready.append(&mut wakers.readers);
                    }

                    if writable {
                        ready.append(&mut wakers.writers);
                    }
                }
            }
            drop(sources);
            for waker in ready.drain(..) {
                waker.wake();
            }
        }
    }
}

fn is_going_to_close(s: TcpState) -> bool {
    match s {
        TcpState::Closed | TcpState::Listen | TcpState::SynSent | TcpState::SynReceived | TcpState::Established => false,
        _ => true,
    }
}
