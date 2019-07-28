extern crate rawsock;

mod shit;
mod rawsock_interface;
mod get_mac;
use get_mac::{GetMac};
use self::shit::{Shit, SthTrait};
use rawsock_interface::RawsockInterface;
use rawsock::{open_best_library};

fn main() {
    let a = Shit::new();
    RawsockInterface::new();
    println!("Hello, world! {}", a.a);

    println!("Opening packet capturing library");
    let lib = open_best_library().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", lib.version());
    let all_interf = lib.all_interfaces().expect("Could not obtain interface list");
    let fuck = all_interf.into_iter().map(|i| {
        let name = i.name;
        println!("Opening the {} interface", &name);
        // Some(lib.open_interface(&name).expect("Could not open network interface"))
        lib.open_interface(&name).ok()
    }).filter(|oi| {
        match oi {
            Some(i) => match i.data_link() {
                rawsock::DataLink::Ethernet => true,
                _ => false
            },
            None => false
        }
    }).map(|i| -> Box<dyn rawsock::traits::Interface> {
        i.unwrap()
    });
    for interf in fuck {
        let mac = interf.get_mac().expect("Could not get mac");
        println!("Interface opened, data link: {}, mac: {}", interf.data_link(), mac);
    }
}
