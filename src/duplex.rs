use std::sync::mpsc;
use std::sync::mpsc::{Receiver, channel, SendError, RecvError, TryRecvError};

pub type Sender<T> = mpsc::Sender<T>;
pub struct ChannelPort<T> {
    sender: Sender::<T>,
    receiver: Receiver::<T>,
}

impl<T> ChannelPort<T> {
    pub fn new() -> (ChannelPort<T>, ChannelPort<T>) {
        let (send1, recv1) = channel::<T>();
        let (send2, recv2) = channel::<T>();
        (
            ChannelPort {
                sender: send1,
                receiver: recv2,
            },
            ChannelPort {
                sender: send2,
                receiver: recv1,
            }
        )
    }
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.sender.send(t)
    }
    pub fn recv(&self) -> Result<T, RecvError> {
        self.receiver.recv()
    }
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.receiver.try_recv()
    }
    pub fn clone_sender(&self) -> Sender::<T> {
        self.sender.clone()
    }
}
