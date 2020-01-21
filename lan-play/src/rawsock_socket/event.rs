use futures::channel::oneshot;
use super::socket::AsyncSocket;

pub(super) enum Event {
    NewSocket(oneshot::Sender<AsyncSocket>),
}
