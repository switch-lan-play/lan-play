extern crate rawsock;

mod shit;
mod rawsock_interface;
mod get_mac;
use rawsock_interface::{CreateDevice, ErrorWithDesc};
use rawsock::{open_best_library};

fn main() {
    println!("Opening packet capturing library");
    let lib = open_best_library().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", lib.version());
    let all_interf = lib.all_interfaces().expect("Could not obtain interface list");
    let (opened, errored): (Vec<_>, _) = all_interf
        .into_iter()
        .map(|i| lib.create_device(i))
        .partition(Result::is_ok);

    for ErrorWithDesc(err, desc) in errored.into_iter().map(|i| i.err().unwrap()) {
        println!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for mut interface in opened.into_iter().map(Result::unwrap) {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
        let interf = &mut interface.interface;
        let _p = interf.receive();
        println!("packet {:?}", _p);
    }
}
