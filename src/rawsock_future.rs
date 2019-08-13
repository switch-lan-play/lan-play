use tokio::reactor;
use tokio::stream::Stream;
use mio::event::Evented;
use mio::{Registration, Token, PollOpt, Ready};
use crate::rawsock_socket::{RawsockInterface, RawsockRunner, RawsockDevice};
use std::io;
use std::thread;
use std::sync::mpsc::TryRecvError;
use std::task;
use log::{warn, debug};
use std::pin::Pin;
use futures::future::poll_fn;
use futures::ready;

pub struct RawsockInterfaceAsync<'a> {
    io: RawsockInterfaceEvented<'a>,
    registration: reactor::Registration,
}

impl<'a> RawsockInterfaceAsync<'a> {
    pub fn new(interf: RawsockInterface<'a>) -> RawsockInterfaceAsync<'a> {
        RawsockInterfaceAsync {
            io: interf.into(),
            registration: reactor::Registration::new(),
        }
    }
    pub async fn recv(&mut self) -> Option<io::Result<Vec<u8>>> {
        poll_fn(|cx| self.poll_recv(cx)).await
    }
    /// Ensure that the I/O resource is registered with the reactor.
    fn register(&self) -> io::Result<()> {
        self.registration
            .register(&self.io)?;
        Ok(())
    }
    fn poll_recv(&mut self, cx: &mut task::Context<'_>) -> task::Poll<Option<io::Result<Vec<u8>>>> {
        self.register()?;
        loop {
            match self.io.inner.port.try_recv() {
                Ok(packet) => return task::Poll::Ready(Some(Ok(packet))),
                Err(TryRecvError::Disconnected) => return task::Poll::Ready(None),
                Err(TryRecvError::Empty) => {
                    ready!(self.registration.poll_read_ready(cx)).unwrap();
                },
            }
        }
    }
}

impl<'a> Stream for RawsockInterfaceAsync<'a> {
    type Item = io::Result<Vec<u8>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Option<Self::Item>> {
        self.register()?;
        self.poll_recv(cx)
    }
}

impl<'a> Into<RawsockInterfaceAsync<'a>> for RawsockInterface<'a> {
    fn into(self) -> RawsockInterfaceAsync<'a> {
        RawsockInterfaceAsync::new(self)
    }
}

pub struct RawsockInterfaceEvented<'a> {
    inner: RawsockDevice,
    runner: RawsockRunner<'a>,
    registration: Registration,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl<'a> Into<RawsockInterfaceEvented<'a>> for RawsockInterface<'a> {
    fn into(self) -> RawsockInterfaceEvented<'a> {
        RawsockInterfaceEvented::new(self)
    }
}

impl<'a> RawsockInterfaceEvented<'a> {
    pub fn new(interf: RawsockInterface<'a>) -> RawsockInterfaceEvented<'a> {
        let (dev, mut runner) = interf.split_device();
        let (registration, s) = Registration::new2();
        let static_runner = hide_lt(&mut runner);

        let sender = runner.port.clone_sender();
        let interf = static_runner.interface.clone();
        let join_handle = Some(thread::spawn(move || {
            let r = interf.loop_infinite_dyn(&|packet| {
                s.set_readiness(Ready::readable()).unwrap();
                match sender.send(packet.as_owned().to_vec()) {
                    Ok(_) => (),
                    Err(err) => warn!("recv error: {:?}", err)
                }
            });
            if !r.is_ok() {
                warn!("loop_infinite {:?}", r);
            }
            debug!("recv thread exit");
        }));
        RawsockInterfaceEvented {
            inner: dev,
            runner,
            registration,
            join_handle,
        }
    }
}

fn hide_lt<'a>(runner: &mut RawsockRunner<'a>) -> &'a mut RawsockRunner<'static> {
    unsafe fn inner<'a>(runner: &mut (RawsockRunner<'a>)) -> &'a mut (RawsockRunner<'static>) {
        use std::mem;
        mem::transmute(runner)
    }
    unsafe { inner(runner) }
}

impl<'a> Drop for RawsockInterfaceEvented<'a> {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            self.runner.interface.break_loop();
            handle.join().unwrap();
        }
    }
}

impl<'a> Evented for RawsockInterfaceEvented<'a> {
    fn register(&self, poll: &mio::Poll, token: Token, interest: Ready, opts: PollOpt)
        -> io::Result<()>
    {
        self.registration.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &mio::Poll, token: Token, interest: Ready, opts: PollOpt)
        -> io::Result<()>
    {
        self.registration.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        self.registration.deregister(poll)
    }
}
