//! Driver framework — PCI probes register `Driver` implementations.

use crate::pci::PciDevice;
use crate::sync::Mutex;
use alloc::vec::Vec;

pub trait Driver {
    fn name(&self) -> &'static str;
    fn probe(&self, dev: &PciDevice) -> bool;
    fn init(&mut self) -> Result<(), &'static str>;
}

static DRIVERS: Mutex<Vec<Box<dyn Driver + Send>>> = Mutex::new(Vec::new());

pub fn register(driver: Box<dyn Driver + Send>) {
    DRIVERS.lock().push(driver);
}

pub fn init_all(devices: &[PciDevice]) {
    for dev in devices {
        for drv in DRIVERS.lock().iter_mut() {
            if drv.probe(dev) {
                serial_println!(
                    "[driver] probing {} for {:04x}:{:04x}",
                    drv.name(),
                    dev.vendor,
                    dev.device
                );
                if let Err(e) = drv.init() {
                    serial_println!("[driver] {} init failed: {}", drv.name(), e);
                } else {
                    serial_println!("[driver] {} attached", drv.name());
                }
            }
        }
    }
}

pub struct NetStub;

impl Driver for NetStub {
    fn name(&self) -> &'static str {
        "net-stub"
    }
    fn probe(&self, _dev: &PciDevice) -> bool {
        false
    }
    fn init(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
}

pub fn init_framework() {
    register(Box::new(NetStub));
    serial_println!("[ ok ] driver framework ready");
}
