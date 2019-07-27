mod shit;
mod rawsock_interface;
use self::shit::{Shit, SthTrait};

fn main() {
    let a = Shit::new();
    println!("Hello, world! {}", a.a);
}
