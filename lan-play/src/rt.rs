#[cfg(feature = "tokio")]
pub use tokio::{
    task::{spawn, JoinHandle},
    select,
    time::{Elapsed, Instant, Duration, timeout, delay_for, interval},
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
        io::{Read as AsyncRead, Write as AsyncWrite},
        net::{TcpStream, TcpListener, UdpSocket},
        sync::{Mutex},
        prelude,
        main,
    };
    pub use std::time::{Instant, Duration};

    use async_std::future;
    use async_std::prelude::*;
    pub fn delay_for(d: Duration) -> impl std::future::Future<Output = ()> {
        future::ready(()).delay(d)
    }
}
#[cfg(feature = "async-std")]
pub use async_std_exports::*;

pub use async_channel::{Sender, Receiver, unbounded as channel, TryRecvError};

pub use std::io;
