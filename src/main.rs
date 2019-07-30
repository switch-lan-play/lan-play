extern crate rawsock;

mod shit;
mod rawsock_interface;
mod get_mac;
use get_mac::{GetMac};
use rawsock_interface::{CreateDevice};
use rawsock::{open_best_library};

fn main() {
    println!("Opening packet capturing library");
    let lib = open_best_library().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", lib.version());
    let all_interf = lib.all_interfaces().expect("Could not obtain interface list");
    let (opened, _errored): (Vec<_>, _) = all_interf
        .into_iter()
        .map(|i| lib.create_device(i))
        .partition(Result::is_ok);

    for mut interface in opened.into_iter().map(Result::unwrap) {
        let name = interface.name();
        let mac = interface.get_mac().expect("Could not get mac");
        println!("Interface {} opened, data link: {}, mac: {}", name, interface.interface.data_link(), mac);
        let interf = &mut interface.interface;
        let _p = interf.receive();
    }
}
