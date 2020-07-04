pub use rawsock::BorrowedPacket;

pub type IntercepterFn = Box<dyn Fn(&BorrowedPacket) -> bool + Send>;

pub struct Intercepter {
    fs: Vec<IntercepterFn>,
}

impl Intercepter {
    pub fn new() -> Intercepter {
        Intercepter {
            fs: vec![]
        }
    }
    pub fn add(mut self, func: impl Fn(&BorrowedPacket) -> bool + Send + 'static) -> Self {
        self.fs.push(Box::new(func));
        self
    }
    pub fn build(self) -> IntercepterFn {
        Box::new(move |packet| {
            for f in &self.fs {
                if f(packet) {
                    return true
                }
            }
            false
        })
    }
}