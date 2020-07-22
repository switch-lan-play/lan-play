pub use rawsock::BorrowedPacket;
use async_channel::Sender;
use super::interface::Packet;

pub type IntercepterFn = Box<dyn Fn(&BorrowedPacket) -> bool + Send + 'static>;
pub type IntercepterFactory = Box<dyn Fn(Sender<Packet>) -> IntercepterFn + Send + 'static>;

enum UnionIntercepter {
    Func(IntercepterFn),
    Factory(IntercepterFactory),
}

pub struct IntercepterBuilder {
    fs: Vec<UnionIntercepter>,
}

impl IntercepterBuilder {
    pub fn new() -> IntercepterBuilder {
        IntercepterBuilder {
            fs: vec![]
        }
    }
    pub fn add(mut self, func: impl Fn(&BorrowedPacket) -> bool + Send + 'static) -> Self {
        self.fs.push(UnionIntercepter::Func(Box::new(func)));
        self
    }
    pub fn add_factory(mut self, factory: impl Fn(Sender<Packet>) -> IntercepterFn + Send + 'static) -> Self {
        self.fs.push(UnionIntercepter::Factory(Box::new(factory)));
        self
    }
    pub fn build(self, sender: Sender<Packet>) -> IntercepterFn {
        let fs: Vec<IntercepterFn> = self.fs.into_iter().map(|i| match i {
            UnionIntercepter::Func(f) => f,
            UnionIntercepter::Factory(f) => f(sender.clone()),
        }).collect();
        Box::new(move |packet| {
            for f in &fs {
                if f(packet) {
                    return true
                }
            }
            false
        })
    }
}