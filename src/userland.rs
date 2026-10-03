//! User program loading — flat binary or ELF via `process::spawn_elf`.

pub use crate::process::Process;
pub use crate::elf::{load, ElfError, LoadedElf};

pub const USER_TEXT_BASE: u64 = 0x0040_0000;

/// Load an ELF image and spawn a user process.
pub fn spawn_elf(name: &'static str, image: &'static [u8]) -> Result<u64, ElfError> {
    Process::spawn_elf(name, image)
}
