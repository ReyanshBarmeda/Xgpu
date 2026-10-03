//! Networking — driver hooks only (virtio-net later).

pub trait NetDriver {
    fn name(&self) -> &'static str;
    fn send(&mut self, _packet: &[u8]) -> Result<(), &'static str> {
        Err("not implemented")
    }
}

pub fn init() {
    serial_println!("[ ok ] net layer stub (no NIC bound)");
}
