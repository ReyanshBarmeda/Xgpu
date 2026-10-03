#![no_std]
#![feature(abi_x86_interrupt)]

//! # Xdos
//!
//! A 64-bit x86 kernel. boot.S takes the CPU from the 32-bit protected
//! mode GRUB hands us into long mode, then calls `kernel_main` here.

extern crate alloc;

pub mod address_space;
pub mod allocator;
pub mod boot_info;
pub mod cpu;
pub mod driver;
pub mod elf;
pub mod framebuffer;
pub mod gdt;
pub mod gui;
pub mod interrupts;
pub mod ipc;
pub mod keyboard;
pub mod memory;
pub mod mouse;
pub mod net;
pub mod pci;
pub mod pic;
pub mod pit;
pub mod process;
pub mod scheduler;
pub mod security;
pub mod serial;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod userland;
pub mod vga_buffer;
pub mod vfs;

use alloc::vec::Vec;
use core::panic::PanicInfo;
use vga_buffer::Color;

/// Entry point, called from boot/long_mode_start.S.
///
/// `multiboot_info_addr` is the pointer GRUB left in ebx, forwarded
/// through the boot stub.
#[no_mangle]
pub extern "C" fn kernel_main(multiboot_info_addr: u64) -> ! {
    serial::init();

    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.clear_screen();
        writer.set_color(Color::LightCyan, Color::Black);
    }

    // Boot banner in solid CP437 block glyphs (0xDB). Written as raw
    // bytes rather than through println!, because the formatting path
    // folds non-ASCII down to a placeholder glyph.
    //
    // Stored one row per entry rather than as a single literal with
    // backslash-newline continuations: those eat the following line's
    // leading whitespace, which would strip the centring padding.
    const BANNER: [&[u8]; 6] = [
        b"                       \xDB\xDB   \xDB\xDB  \xDB\xDB\xDB\xDB\xDB\xDB    \xDB\xDB\xDB\xDB\xDB    \xDB\xDB\xDB\xDB\xDB\xDB",
        b"                        \xDB\xDB \xDB\xDB   \xDB\xDB   \xDB\xDB  \xDB\xDB   \xDB\xDB  \xDB\xDB",
        b"                         \xDB\xDB\xDB    \xDB\xDB   \xDB\xDB  \xDB\xDB   \xDB\xDB   \xDB\xDB\xDB\xDB\xDB",
        b"                         \xDB\xDB\xDB    \xDB\xDB   \xDB\xDB  \xDB\xDB   \xDB\xDB       \xDB\xDB",
        b"                        \xDB\xDB \xDB\xDB   \xDB\xDB   \xDB\xDB  \xDB\xDB   \xDB\xDB  \xDB\xDB   \xDB\xDB",
        b"                       \xDB\xDB   \xDB\xDB  \xDB\xDB\xDB\xDB\xDB\xDB    \xDB\xDB\xDB\xDB\xDB    \xDB\xDB\xDB\xDB\xDB",
    ];

    cpu::without_interrupts(|| {
        let mut writer = vga_buffer::WRITER.lock();
        writer.write_byte(b'\n');
        for row in BANNER {
            writer.write_bytes(row);
            writer.write_byte(b'\n');
        }
        writer.set_color(Color::LightGray, Color::Black);
        writer.write_byte(b'\n');
    });

    serial_println!(concat!(
        "\n",
        "                       ##   ##  ######    #####    ######\n",
        "                        ## ##   ##   ##  ##   ##  ##     \n",
        "                         ###    ##   ##  ##   ##   ##### \n",
        "                         ###    ##   ##  ##   ##       ##\n",
        "                        ## ##   ##   ##  ##   ##  ##   ##\n",
        "                       ##   ##  ######    #####    ##### \n",
    ));
    serial_println!("[xdos] serial up, entering init");

    cpu::init_boot();

    unsafe {
        boot_info::init(multiboot_info_addr);
    }

    // Order matters here.
    //
    // The GDT comes first because the IDT's gate descriptors reference
    // the kernel code selector it installs, and the double-fault gate
    // references the TSS's IST stack.
    gdt::init();
    println!("[ ok ] GDT + TSS loaded (incl. ring-3 segments)");

    // Then the IDT, so that any fault from this point on is reported
    // rather than silently triple-faulting.
    interrupts::init_idt();
    println!("[ ok ] IDT loaded");

    // Remap the PIC before enabling interrupts, or IRQ0 would arrive on
    // vector 8 and be mistaken for a double fault.
    pic::init();
    pit::init();
    println!("[ ok ] PIC remapped, PIT at {} Hz", pit::TICK_HZ);

    // Physical memory must be up before the heap, which needs frames.
    unsafe { memory::init(multiboot_info_addr) };
    let (used, total) = memory::stats();
    println!(
        "[ ok ] frame allocator: {} KiB usable, {} KiB in use",
        total * 4,
        used * 4
    );

    match allocator::init() {
        Ok(()) => println!("[ ok ] kernel heap: {} KiB mapped", allocator::HEAP_SIZE / 1024),
        Err(e) => panic!("heap initialization failed: {:?}", e),
    }

    syscall::init();
    vfs::init();
    ipc::init();
    driver::init_framework();
    let pci_devices = pci::init();
    driver::init_all(&pci_devices);
    mouse::init();
    framebuffer::init_from_boot();
    gui::init();
    net::init();

    // Interrupts stay masked until every handler is in place.
    cpu::enable_interrupts();
    println!("[ ok ] interrupts enabled");

    self_test();

    // Everything from here on runs as scheduled tasks, not as
    // kernel_main's own code — scheduler::start() never returns to this
    // stack. The counter tasks exist only to make multitasking visible:
    // their interleaved serial output is the proof the scheduler is
    // actually switching between them, not just running one to
    // completion.
    scheduler::spawn("shell", shell_task, 0).expect("failed to spawn shell task");
    scheduler::spawn("counter-A", counter_task, b'A' as u64).expect("failed to spawn counter-A");
    scheduler::spawn("counter-B", counter_task, b'B' as u64).expect("failed to spawn counter-B");

    println!(
        "[ ok ] {} tasks spawned, handing off to scheduler",
        scheduler::task_count()
    );

    unsafe { scheduler::start() }
}

/// The interactive task: echoes keystrokes to the VGA screen. This is
/// what used to be kernel_main's own final loop before it became a
/// proper task.
extern "C" fn shell_task(_arg: u64) -> ! {
    println!();
    println!("Xdos ready. Type something:");
    println!();

    loop {
        mouse::poll();
        while let Some(ch) = keyboard::pop_char() {
            print!("{}", ch as char);
        }
        cpu::hlt();
    }
}

/// A demo task proving the scheduler actually interleaves execution.
/// `arg` is an ASCII byte identifying which instance this is, so two
/// copies of the same function print distinguishably.
extern "C" fn counter_task(arg: u64) -> ! {
    let label = arg as u8 as char;
    let mut count: u64 = 0;
    loop {
        serial_println!("[task {}] tick {}", label, count);
        count += 1;
        pit::sleep_ms(1000);
    }
}

/// Exercise each subsystem once so a broken one shows up at boot rather
/// than at some random later moment.
fn self_test() {
    // The IDT: a breakpoint should be caught and returned from.
    interrupts::trigger_breakpoint();

    // The heap: allocation, growth, and drop.
    let mut v: Vec<u64> = Vec::new();
    for i in 0..256 {
        v.push(i * i);
    }
    assert_eq!(v.len(), 256);
    assert_eq!(v[255], 255 * 255);
    drop(v);

    // Reallocating after the drop proves memory was actually reclaimed
    // rather than leaked.
    let v2: Vec<u8> = alloc::vec![0xAB; 4096];
    assert_eq!(v2.len(), 4096);
    drop(v2);

    // Paging: the heap's virtual address must translate to a real frame.
    let translated = memory::translate(allocator::HEAP_START);
    assert!(translated.is_some(), "heap page is not mapped");

    println!("[ ok ] self-test passed");

    // The timer: wait for ticks to confirm IRQ0 is actually firing.
    let before = pit::ticks();
    pit::sleep_ms(50);
    let after = pit::ticks();
    if after > before {
        println!("[ ok ] timer ticking ({} ticks in ~50 ms)", after - before);
    } else {
        println!("[warn] timer is not ticking - check PIC/PIT wiring");
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // A fault can happen while this very task already holds the VGA or
    // serial lock (e.g. a page fault triggered by code inside println!
    // itself). Without this, that would deadlock the panic handler
    // against itself, forever, with no other task able to release the
    // lock and no output ever printed. Forcibly freeing both locks
    // right before use is a deliberate trade: the data behind them
    // might be left mid-update, but the alternative is silence.
    unsafe {
        vga_buffer::WRITER.force_unlock();
        serial::SERIAL1.force_unlock();
    }

    // Print to both sinks: serial survives a scrolled-off screen, and
    // VGA is visible without a serial console attached.
    serial_println!("[xdos] KERNEL PANIC: {}", info);

    cpu::without_interrupts(|| {
        use core::fmt::Write;
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(Color::White, Color::Red);
        let _ = write!(writer, "\nKERNEL PANIC: {}\n", info);
    });

    cpu::halt_loop();
}
