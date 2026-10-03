//! User pointer checks for syscalls.

use crate::scheduler;
use crate::task::TaskKind;

/// Verify `[ptr, ptr+len)` lies in a user mapping when the caller is ring 3.
pub fn verify_user_buffer(ptr: u64, len: u64) -> Result<(), i64> {
    if len == 0 {
        return Ok(());
    }
    if ptr == 0 {
        return Err(-14); // EFAULT
    }
    let end = ptr.saturating_add(len).saturating_sub(1);
    if end < ptr {
        return Err(-14);
    }

    if scheduler::current_process_kind() != Some(TaskKind::User) {
        return Ok(());
    }

    // User regions: load segments below 512 MiB and stack near USER_STACK_TOP.
    const USER_LO: u64 = 0x0000_0000_0040_0000;
    const USER_HI: u64 = 0x0000_0000_0080_0000 + 64 * 1024;
    if ptr >= USER_LO && end < USER_HI {
        return Ok(());
    }
    Err(-14)
}
