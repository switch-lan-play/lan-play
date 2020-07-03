use super::{Ethernet, SocketHandle, SocketSet};
use futures::prelude::*;
use futures::select;
use smoltcp::time::{Duration, Instant};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Poll, Waker};
use tokio::sync::Notify;

use tokio::time::delay_for;

#[derive(Debug)]
pub struct Wakers {
    readers: Vec<Waker>,
    writers: Vec<Waker>,
}

pub(super) struct Source {
    wakers: Mutex<Wakers>,
}

#[derive(Clone)]
pub(super) struct NetReactor {
    socket_set: Arc<Mutex<SocketSet>>,
    sources: Arc<Mutex<HashMap<SocketHandle, Arc<Source>>>>,
    notify: Arc<Notify>,
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
    pub fn new(socket_set: Arc<Mutex<SocketSet>>) -> NetReactor {
        NetReactor {
            socket_set,
            sources: Arc::new(Mutex::new(HashMap::new())),
            notify: Arc::new(Notify::new()),
        }
    }
    pub async fn lock_set(&self) -> MutexGuard<'_, SocketSet> {
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
        let default_timeout = Duration::from_millis(1000);
        let sockets = self.socket_set.clone();

        loop {
            let start = Instant::now();
            let deadline = {
                ethernet
                    .poll_delay(sockets.lock().unwrap().as_set_mut(), start)
                    .unwrap_or(default_timeout)
            };
            let device = ethernet.device_mut();

            select! {
                _ = delay_for(deadline.into()).fuse() => {},
                _ = device.receiver.peek().fuse() => {},
                _ = self.notify.notified().fuse() => {},
            }
            let mut set = sockets.lock().unwrap();
            let end = Instant::now();
            let readiness = match ethernet.poll(set.as_set_mut(), end) {
                Ok(b) => b,
                Err(smoltcp::Error::Dropped) => false,
                Err(e) => {
                    log::error!("poll error {:?}", e);
                    true
                }
            };

            if !readiness {
                continue;
            }

            let mut ready = Vec::new();
            let sources = self.sources.lock().unwrap();
            for socket in set.as_set_mut().iter() {
                let (readable, writable) = match socket {
                    smoltcp::socket::Socket::Tcp(tcp) => (tcp.can_recv() || !tcp.is_open(), tcp.can_send()),
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
            for waker in ready {
                waker.wake();
            }
        }
    }
}
