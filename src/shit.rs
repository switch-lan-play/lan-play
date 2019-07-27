pub struct Shit {
    pub a: i32
}

pub trait SthTrait {
    fn new() -> Shit;
}

impl SthTrait for Shit {
    fn new() -> Shit {
        Shit {
            a: 1
        }
    }
}
