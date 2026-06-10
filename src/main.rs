#![no_std]
#![no_main]

use core::fmt::Write;
use core::panic::PanicInfo;

mod multiboot;
mod serial;
mod io;
mod pmm;

// the entry point
core::arch::global_asm!(
    r#"
    .section .text
    .global _start
_start:
    // rust code needs valid stack registers
    lea esp, [stack_top]

    // multiboot magic and info args
    push ebx
    push eax
    call kernel_main

1:
    hlt
    jmp 1b

    .section .bss
    .align 16
stack_bottom:
    // boot stack; used until the memory management is initialized
    .skip 16384
stack_top:
"#
);

const MULTIBOOT_MAGIC: u32 = 0x1BAD_B002;
const MULTIBOOT_FLAGS: u32 = 0x0000_0003;
const MULTIBOOT_CHECKSUM: u32 = 0u32.wrapping_sub(MULTIBOOT_MAGIC + MULTIBOOT_FLAGS);

// add the header to the .multiboot linker section, so grub knows the kernel is bootable
#[used]
#[link_section = ".multiboot"]
static MULTIBOOT_HEADER: [u32; 3] = [MULTIBOOT_MAGIC, MULTIBOOT_FLAGS, MULTIBOOT_CHECKSUM];

fn alloc_print() -> u32 {
    let mut serial = serial::SerialIO;
    serial.init();
    if let Some(page) = pmm::alloc_page() {
        writeln!(serial, "allocated page: 0x{page:08x}").ok();
        return page;
    } else {
        return 0;
    }
}

#[no_mangle] // stops the compiler from renaming the function
pub extern "C" fn kernel_main(_multiboot_magic: u32, _multiboot_info: u32) -> ! {
    let mut serial = serial::SerialIO;
    serial.init();
    writeln!(serial, "kernel booted; serial logging works").ok();
    writeln!(serial, "blehhhh").ok();
    
    let regions = multiboot::parse_memory_map(_multiboot_info);
    pmm::init_pages(&regions);

    writeln!(serial, "Found {} usable regions", regions.count).ok();

    for i in 0..regions.count {
        let (base, len) = regions.regions[i];
        writeln!(serial, "  [{i}] base=0x{base:08X} len=0x{len:08X} ({len} bytes)").ok();
    }

    writeln!(serial, "free ram starts at 0x{:08X}", pmm::free_ram_start()).ok();

    if let Some(page) = pmm::alloc_page() {
        writeln!(serial, "allocated page: 0x{page:08x}").ok();
    } else {
        writeln!(serial, "failed to allocate page").ok();
    }

    let a1 = alloc_print();
    let a2 = alloc_print();
    let a3 = alloc_print();

    pmm::free_page(a2);

    let a2 = alloc_print();

    echo_input()
    // halt()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut writer = serial::SerialIO;
    writer.init();
    writeln!(writer, "Kernel panic:").ok();
    writeln!(writer, "{info}").ok();

    halt()
}

fn echo_input() -> ! {
    let mut ser = serial::SerialIO;
    ser.init();
    loop {
        let byte = ser.read_byte();
        if byte == b'\r' {
            ser.write_byte(b'\n');
        }
        ser.write_byte(byte);
    }
}

fn halt() -> ! {
    loop {
        unsafe {
            // sleep until the next interrupt
            //
            // there's no interrupt handling yet, so the CPU just hangs
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

