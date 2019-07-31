extern crate rawsock;
extern crate smoltcp;

mod rawsock_interface;
mod get_mac;
use rawsock_interface::{ErrorWithDesc, RawsockInterfaceSet};
use rawsock::{open_best_library};

fn main() {
    println!("Opening packet capturing library");
    let set = RawsockInterfaceSet::new().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", set.lib_version());
    let (opened, errored): (Vec<_>, _) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        println!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for mut interface in opened {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
    }
}
