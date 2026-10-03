//! PCI config space (port I/O).

use crate::cpu::{inb, outb, io_wait};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

#[derive(Clone, Copy, Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor: u16,
    pub device: u16,
    pub class: u8,
    pub subclass: u8,
}

fn config_read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    unsafe {
        let address = 0x8000_0000u32
            | u32::from(bus) << 16
            | u32::from(slot) << 11
            | u32::from(func) << 8
            | u32::from(offset & 0xFC);
        outb(CONFIG_ADDRESS, (address >> 24) as u8);
        outb(CONFIG_ADDRESS + 1, (address >> 16) as u8);
        outb(CONFIG_ADDRESS + 2, (address >> 8) as u8);
        outb(CONFIG_ADDRESS + 3, address as u8);
        io_wait();
        let b0 = inb(CONFIG_DATA) as u32;
        let b1 = inb(CONFIG_DATA + 1) as u32;
        let b2 = inb(CONFIG_DATA + 2) as u32;
        let b3 = inb(CONFIG_DATA + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }
}

pub fn scan() -> alloc::vec::Vec<PciDevice> {
    let mut found = alloc::vec::Vec::new();
    for bus in 0..=0 {
        for slot in 0..32 {
            for func in 0..8 {
                let reg = config_read32(bus, slot, func, 0);
                let vendor = (reg & 0xFFFF) as u16;
                if vendor == 0xFFFF {
                    continue;
                }
                let device = (reg >> 16) as u16;
                let class_reg = config_read32(bus, slot, func, 0x08);
                let class = ((class_reg >> 24) & 0xFF) as u8;
                let subclass = ((class_reg >> 16) & 0xFF) as u8;
                found.push(PciDevice {
                    bus,
                    slot,
                    func,
                    vendor,
                    device,
                    class,
                    subclass,
                });
                if func == 0 && (reg >> 16 & 0x80) == 0 {
                    break;
                }
            }
        }
    }
    found
}

pub fn init() -> alloc::vec::Vec<PciDevice> {
    let devices = scan();
    serial_println!("[ ok ] PCI: {} device(s)", devices.len());
    for d in &devices {
        serial_println!(
            "       {:02x}:{:02x}.{} {:04x}:{:04x} class {:02x}/{:02x}",
            d.bus,
            d.slot,
            d.func,
            d.vendor,
            d.device,
            d.class,
            d.subclass
        );
    }
    devices
}
