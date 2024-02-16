#![no_main]
#![no_std]

use log::info;
use uefi::prelude::*;

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    info!("Hello world!");

    loop {
        x86_64::instructions::hlt()
    }
}
