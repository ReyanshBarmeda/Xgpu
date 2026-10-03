//! Minimal ELF64 loader (`ET_EXEC`, `PT_LOAD`).

use crate::address_space::AddressSpace;
use crate::memory::{self, MapError, PAGE_SIZE};

const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;
const EM_X86_64: u16 = 62;

#[derive(Debug)]
pub enum ElfError {
    BadMagic,
    Not64,
    NotExec,
    WrongArch,
    BadPhdr,
    Map(MapError),
}

#[repr(C)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

pub struct LoadedElf {
    pub entry: u64,
}

pub fn load(aspace: &AddressSpace, image: &[u8]) -> Result<LoadedElf, ElfError> {
    if image.len() < core::mem::size_of::<Elf64Ehdr>() {
        return Err(ElfError::BadMagic);
    }
    let ehdr = unsafe { &*(image.as_ptr() as *const Elf64Ehdr) };
    if ehdr.e_ident[..4] != ELF_MAGIC {
        return Err(ElfError::BadMagic);
    }
    if ehdr.e_ident[4] != 2 {
        return Err(ElfError::Not64);
    }
    if ehdr.e_type != 2 && ehdr.e_type != 3 {
        return Err(ElfError::NotExec);
    }
    if ehdr.e_machine != EM_X86_64 {
        return Err(ElfError::WrongArch);
    }
    if ehdr.e_phentsize as usize != core::mem::size_of::<Elf64Phdr>() {
        return Err(ElfError::BadPhdr);
    }

    for i in 0..ehdr.e_phnum as usize {
        let off = ehdr.e_phoff as usize + i * ehdr.e_phentsize as usize;
        if off + core::mem::size_of::<Elf64Phdr>() > image.len() {
            return Err(ElfError::BadPhdr);
        }
        let ph = unsafe { &*(image.as_ptr().add(off) as *const Elf64Phdr) };
        if ph.p_type != PT_LOAD {
            continue;
        }
        load_segment(aspace, image, ph)?;
    }

    Ok(LoadedElf { entry: ehdr.e_entry })
}

fn load_segment(aspace: &AddressSpace, image: &[u8], ph: &Elf64Phdr) -> Result<(), ElfError> {
    let virt_start = ph.p_vaddr & !(PAGE_SIZE - 1);
    let virt_end = (ph.p_vaddr + ph.p_memsz + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let executable = ph.p_flags & 1 != 0;

    let mut virt = virt_start;
    while virt < virt_end {
        let frame = memory::FRAME_ALLOCATOR
            .lock()
            .allocate()
            .ok_or(ElfError::Map(MapError::OutOfFrames))?;
        aspace
            .map_user(virt, frame, executable)
            .map_err(ElfError::Map)?;
        virt += PAGE_SIZE;
    }

    let file_end = ph.p_offset + ph.p_filesz;
    if file_end as usize > image.len() {
        return Err(ElfError::BadPhdr);
    }
    let chunk = &image[ph.p_offset as usize..(ph.p_offset + ph.p_filesz) as usize];
    aspace.write_bytes(ph.p_vaddr, chunk);
    if ph.p_memsz > ph.p_filesz {
        let zero_len = (ph.p_memsz - ph.p_filesz) as usize;
        let mut zeros = alloc::vec![0u8; zero_len];
        aspace.write_bytes(ph.p_vaddr + ph.p_filesz, &zeros);
    }
    Ok(())
}
