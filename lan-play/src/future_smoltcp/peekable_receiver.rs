use crate::rt::{TryRecvError, Receiver};

pub struct PeekableReceiver<T> {
    temp: Option<T>,
    receiver: Receiver<T>,
}

impl<T> PeekableReceiver<T> {
    pub fn new(receiver: Receiver<T>) -> Self {
        Self {
            temp: None,
            receiver,
        }
    }
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        if let Some(t) = self.temp.take() {
            return Ok(t);
        }
        self.receiver.try_recv()
    }
    // pub fn try_peek_recv(&mut self) -> Result<&mut Option<T>, TryRecvError> {
    //     match self.receiver.try_recv() {
    //         Ok(i) => {
    //             self.temp = Some(i);
    //             Ok(&mut self.temp)
    //         },
    //         Err(TryRecvError::Empty) => {
    //             self.temp = None;
    //             Ok(&mut self.temp)
    //         },
    //         Err(TryRecvError::Closed) => {
    //             Err(TryRecvError::Closed)
    //         },
    //     }
    // }
    pub async fn peek(&mut self) -> &Option<T> {
        self.temp = self.receiver.recv().await;
        &self.temp
    }
}
