//! Simple kernel message queue IPC.

use crate::sync::Mutex;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

struct Queue {
    messages: VecDeque<Vec<u8>>,
    max: usize,
}

static Q: Mutex<Queue> = Mutex::new(Queue {
    messages: VecDeque::new(),
    max: 32,
});

pub fn init() {
    serial_println!("[ ok ] IPC queue ({} slots)", Q.lock().max);
}

pub fn send(data: &[u8]) -> Result<(), i64> {
    let mut q = Q.lock();
    if q.messages.len() >= q.max {
        return Err(-11); // EAGAIN
    }
    q.messages.push_back(data.to_vec());
    Ok(())
}

pub fn recv(buf: &mut [u8]) -> Result<usize, i64> {
    let mut q = Q.lock();
    let Some(msg) = q.messages.pop_front() else {
        return Ok(0);
    };
    let n = msg.len().min(buf.len());
    buf[..n].copy_from_slice(&msg[..n]);
    Ok(n)
}
