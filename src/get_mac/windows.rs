extern crate libc;
use super::{MacAddressError};
use smoltcp::wire::{EthernetAddress};
use std::{mem, ptr};

const NO_ERROR: u32 = 0;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

fn get_guid<'a>(s: &'a String) -> Option<&'a str> {
    if let Some(pos) = s.find('{') {
        let p = pos + 1;
        if let Some(end) = s[p..].find('}') {
            return Some(&s[p..(p + end)]);
        }
    }
    return None;
}

fn from_u16(s: &[u16]) -> Option<String> {
    if let Some(pos) = s.iter().position(|c| *c == 0) {
        if let Ok(string) =
            String::from_utf16(&s[0..pos])
        {
            return Some(string);
        }
    }
    return None;
}

pub fn get_mac(name: &String) -> Result<EthernetAddress, MacAddressError> {
    if let Some(intf_guid) = get_guid(name) {
        let mut size = 0u32;
        let mut table: *mut MibIftable = ptr::null_mut();

        unsafe {
            if GetIfTable(
                ptr::null_mut::<MibIftable>(),
                &mut size as *mut libc::c_ulong,
                false,
            ) == ERROR_INSUFFICIENT_BUFFER
            {
                table = mem::transmute(libc::malloc(size as libc::size_t));
            }

            if GetIfTable(table, &mut size as *mut libc::c_ulong, false) == NO_ERROR {
                let ptr: *const MibIfrow = (&(*table).table) as *const _;
                let table = std::slice::from_raw_parts(
                    ptr,
                    (*table).dw_num_entries as usize
                );
                for i in table {
                    let row = &*i;

                    if let Some(name) = from_u16(&row.wsz_name) {
                        if let Some(guid) = get_guid(&name) {
                            if guid == intf_guid {
                                let len = row.dw_phys_addr_len;
                                if len == 6 {
                                    return Ok(EthernetAddress::from_bytes(&row.b_phys_addr[0..6]));
                                }
                            }
                        }
                    }
                }
            }
            libc::free(mem::transmute(table));
        }
        return Err(MacAddressError::NotFound);
    }
    Err(MacAddressError::NotFound)
}

pub const MAX_INTERFACE_NAME_LEN: usize = 256;
pub const MAXLEN_PHYSADDR: usize = 8;
pub const MAXLEN_IFDESCR: usize = 256;

#[repr(C)]
pub(crate) struct MibIfrow {
    pub wsz_name: [u16; MAX_INTERFACE_NAME_LEN],
    pub dw_index: u32,
    pub dw_type: u32,
    pub dw_mtu: u32,
    pub dw_speed: u32,
    pub dw_phys_addr_len: u32,
    pub b_phys_addr: [u8; MAXLEN_PHYSADDR],
    pub _padding1: [u8; 16 * 4],
    pub _padding2: [u8; MAXLEN_IFDESCR],
}

#[repr(C)]
pub(crate) struct MibIftable {
    pub dw_num_entries: u32,
    pub table: MibIfrow,
}

#[link(name = "iphlpapi")]
#[allow(non_snake_case)]
extern "system" {
    pub(crate) fn GetIfTable(
        table: *mut MibIftable,
        size: *mut libc::c_ulong,
        order: bool,
    ) -> u32;
}
