extern crate rawsock;

mod shit;
mod rawsock_interface;
mod get_mac;
use get_mac::GetMac;
use self::shit::{Shit, SthTrait};
use rawsock_interface::RawsockInterface;
use rawsock::{open_best_library};

fn fuck() {
    println!("Opening packet capturing library");
    let lib = open_best_library().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", lib.version());
    let all_interf = lib.all_interfaces().expect("Could not obtain interface list");
    for interf in all_interf {
        let name = interf.name;
        println!("Opening the {} interface", &name);
        let interf = lib.open_interface(&name).expect("Could not open network interface");
        println!("Interface opened, data link: {}", interf.data_link());
    }
}

fn main() {
    let a = Shit::new();
    RawsockInterface::new();
    println!("Hello, world! {}", a.a);
    fuck();
}
