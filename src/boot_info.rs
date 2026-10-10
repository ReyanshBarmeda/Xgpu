//! Multiboot2 information — framebuffer, cmdline, memory tags.

#[derive(Clone, Copy, Debug, Default)]
pub struct FramebufferInfo {
    pub addr: u64,
    pub pitch: u64,
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
}

const TAG_END: u32 = 0;
const TAG_CMDLINE: u32 = 1;
const TAG_FRAMEBUFFER: u32 = 8;

#[derive(Clone, Copy, Debug, Default)]
pub struct BootInfo {
    pub framebuffer: Option<FramebufferInfo>,
    pub cmdline: Option<&'static str>,
}

static mut STORED: BootInfo = BootInfo {
    framebuffer: None,
    cmdline: None,
};

/// Parse tags once during early boot.
///
/// # Safety
/// `info_addr` must be the Multiboot2 pointer from GRUB.
pub unsafe fn init(info_addr: u64) -> BootInfo {
    let mut info = BootInfo::default();
    if info_addr == 0 {
        store(info);
        return info;
    }

    let total_size = *(info_addr as *const u32) as u64;
    let mut tag_addr = info_addr + 8;
    let end = info_addr + total_size;

    while tag_addr < end {
        let tag_type = *(tag_addr as *const u32);
        let tag_size = *((tag_addr + 4) as *const u32) as u64;
        if tag_type == TAG_END {
            break;
        }

        match tag_type {
            TAG_CMDLINE => {
                let s_ptr = (tag_addr + 8) as *const u8;
                if let Some(s) = cstr_to_str(s_ptr) {
                    info.cmdline = Some(s);
                }
            }
            TAG_FRAMEBUFFER => {
                if tag_size >= 32 {
                    let addr = *((tag_addr + 8) as *const u64);
                    let pitch = *((tag_addr + 16) as *const u32) as u64;
                    let width = *((tag_addr + 20) as *const u32);
                    let height = *((tag_addr + 24) as *const u32);
                    let bpp = *((tag_addr + 28) as *const u8);
                    if addr != 0 && width > 0 && height > 0 && bpp >= 8 {
                        info.framebuffer = Some(FramebufferInfo {
                            addr,
                            pitch,
                            width,
                            height,
                            bpp,
                        });
                    }
                }
            }
            _ => {}
        }

        tag_addr += (tag_size + 7) & !7;
    }

    store(info);
    info
}

pub fn get() -> BootInfo {
    unsafe { STORED }
}

unsafe fn store(info: BootInfo) {
    STORED = info;
}

unsafe fn cstr_to_str(ptr: *const u8) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
        if len > 4096 {
            return None;
        }
    }
    let slice = core::slice::from_raw_parts(ptr, len);
    core::str::from_utf8(slice).ok()
}
