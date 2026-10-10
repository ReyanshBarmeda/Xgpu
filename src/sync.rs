//! A minimal spinlock. Xdos has no dependencies, so this stands in for
//! the `spin` crate.
//!
//! Important: locking one of these inside an interrupt handler while the
//! interrupted code already holds it will deadlock forever (there is no
//! scheduler to switch away to). Callers that share a lock with an
//! interrupt handler must wrap access in `cpu::without_interrupts`.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// Safe because the AtomicBool guarantees only one guard hands out a
// reference to `data` at a time.
unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Mutex {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Forcibly mark this lock as free, without regard for who holds it.
    ///
    /// This exists for exactly one caller: the panic handler. If a fault
    /// happens while, say, the VGA writer lock is already held, the
    /// panic handler's own attempt to print would deadlock forever
    /// against itself — there is no other thread that will ever release
    /// it. Calling this immediately before panic output is a deliberate
    /// admission that the data behind the lock might be left mid-update,
    /// which is acceptable because the alternative is printing nothing
    /// at all and reasoning about a fault with zero information.
    ///
    /// # Safety
    /// Must only be called when a lock is suspected to be held forever
    /// with no other path to release it (i.e. on the panic path). Using
    /// it in any other situation reintroduces the exact data race the
    /// lock exists to prevent.
    pub unsafe fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin on a plain read until it looks free again; this keeps
            // the cache line shared instead of bouncing it exclusively.
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        MutexGuard { mutex: self }
    }
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}
