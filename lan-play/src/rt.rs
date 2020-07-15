#[cfg(feature = "tokio")]
pub use tokio::{
    task::{spawn, JoinHandle},
    select,
    time::{Elapsed, Instant, Duration, timeout, delay_for, Delay, interval},
    io::{AsyncRead, AsyncWrite, BufWriter, split, copy},
    sync::{Mutex, Notify},
    prelude,
    net::{UdpSocket, TcpStream, TcpListener},
    main,
    test,
};
#[cfg(feature = "async-std")]
mod async_std_exports {
    pub use async_std::{
        task::{spawn, JoinHandle},
        future::{timeout, TimeoutError as Elapsed},
        stream::{interval},
        io::{Read as AsyncRead, Write as AsyncWrite, BufWriter, copy},
        net::{TcpStream, TcpListener, UdpSocket},
        sync::{Mutex},
        prelude,
        main,
        test
    };
    pub use std::time::{Instant, Duration};

    use async_std::future;
    use async_std::prelude::*;
    use futures::io::{ReadHalf, WriteHalf, AsyncReadExt};
    pub fn split<T>(stream: T) -> (ReadHalf<T>, WriteHalf<T>)
    where
        T: AsyncRead + AsyncWrite,
    {
        stream.split()
    }
    pub fn delay_for(d: Duration) -> impl std::future::Future<Output = ()> {
        future::ready(()).delay(d)
    }
    use async_channel::{Sender, Receiver, unbounded as channel};
    pub struct Notify {
        rx: Receiver<()>,
        tx: Sender<()>
    }
    impl Notify {
        pub fn new() -> Notify {
            let (tx, rx) = channel();
            Notify {
                rx,
                tx,
            }
        }
        pub async fn notified(&self) {
            self.rx.recv().await.unwrap()
        }
        pub fn notify(&self) {
            self.tx.try_send(()).unwrap();
        }
    }
}
#[cfg(feature = "async-std")]
pub use async_std_exports::*;

pub use async_channel::{Sender, Receiver, unbounded as channel, TryRecvError};

pub use std::io;
