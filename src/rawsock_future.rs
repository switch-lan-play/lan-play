use tokio::io::{AsyncRead, AsyncWrite};
use tokio::reactor::PollEvented;
use mio::event::Evented;
use mio::{Registration, Poll, Token, PollOpt, Ready};
use crate::rawsock_interface::{RawsockInterface, RawsockRunner, RawsockDevice};
use std::io;
use std::thread;
use std::sync::mpsc::TryRecvError;
use log::{warn, debug};

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
        let raw_runner = hide_lt(&mut runner);

        let sender = runner.port.clone_sender();
        let interf = unsafe { (*raw_runner).interface.clone() };
        let join_handle = Some(thread::spawn(move || {
            let r = interf.borrow().loop_infinite_dyn(&|packet| {
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

fn hide_lt<'a>(runner: &mut RawsockRunner<'a>) -> *mut RawsockRunner<'static> {
    unsafe fn inner<'a>(runner: *mut (RawsockRunner<'a>)) -> *mut (RawsockRunner<'static>) {
        use std::mem;
        // false positive: https://github.com/rust-lang/rust-clippy/issues/2906
        #[allow(clippy::transmute_ptr_to_ptr)]
        mem::transmute(runner)
    }
    unsafe { inner(runner as *mut _) }
}

impl<'a> Drop for RawsockInterfaceEvented<'a> {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            self.runner.interface.borrow().break_loop();
            handle.join().unwrap();
        }
    }
}

impl<'a> Evented for RawsockInterfaceEvented<'a> {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
        -> io::Result<()>
    {
        self.registration.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
        -> io::Result<()>
    {
        self.registration.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.registration.deregister(poll)
    }
}

impl<'a> io::Read for RawsockInterfaceEvented<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.port.try_recv() {
            Err(err) => Err(io::Error::new(io::ErrorKind, error: E))
        }
    }
}