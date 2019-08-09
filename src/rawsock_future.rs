use tokio::io::{AsyncRead, AsyncWrite};
use tokio::reactor::PollEvented;
use mio::event::Evented;
use mio::{Registration, Poll, Token, PollOpt, Ready};
use crate::rawsock_interface::{RawsockInterface};
use std::io;

pub type Shit<'a> = PollEvented<RawsockInterfaceEvented<'a>>;

pub struct RawsockInterfaceEvented<'a> {
    inner: RawsockInterface<'a>,
    registration: Registration,
}

impl<'a> Into<RawsockInterfaceEvented<'a>> for RawsockInterface<'a> {
    fn into(self) -> RawsockInterfaceEvented<'a> {
        RawsockInterfaceEvented::new(self)
    }
}

impl<'a> RawsockInterfaceEvented<'a> {
    pub fn new(inner: RawsockInterface<'a>) -> RawsockInterfaceEvented<'a> {
        let (registration, set_readiness) = Registration::new2();
        RawsockInterfaceEvented {
            inner,
            registration,
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

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        self.registration.deregister(poll)
    }
}
