# OSDev From Scratch: A Guide

This guide walks through building a tiny Rust kernel that boots via GRUB
Multiboot, prints to VGA and serial, and grows into a real OS one feature at a
time. Each section explains what to build, why, and where the sharp edges are.

The project lives in `src/main.rs`, `linker.ld`, and the files referenced
below. Read them alongside this guide.

## 1. Start With the Smallest Thing

Before interrupts, before paging, before anything complicated: build a binary
that boots, writes text, and halts. If that works, everything else is just
adding one piece at a time.

Your first kernel should:

1. Produce a freestanding binary (no OS underneath).
2. Let GRUB load and launch it.
3. Write text to VGA memory.
4. Halt the CPU.

If you see your message on screen, you've proven the machine reached your Rust
code with no operating system, no standard library, and no runtime.

## 2. Freestanding Rust

A normal Rust program expects an OS to provide process startup, memory,
files, and stdout. A kernel has none of that. Start `src/main.rs` with:

```rust
#![no_std]
#![no_main]
```

`no_std` switches from the full standard library to `core` (which needs no OS).
`no_main` tells Rust not to expect the usual `fn main()` entry path — you'll
provide your own via an assembly stub.

## 3. Bootloader: GRUB + Multiboot

GRUB is the bootloader. It starts from firmware, loads your kernel from the
ISO, validates the Multiboot header, and jumps to your entry point.

**GRUB config** (`iso/boot/grub/grub.cfg`):

```cfg
insmod multiboot
multiboot /boot/kernel.bin
boot
```

`multiboot /boot/kernel.bin` tells GRUB this is a Multiboot v1 kernel.
Booting directly (no menuentry) simplifies headless QEMU verification.

## 4. Multiboot Header

GRUB scans the first 8 KiB of your kernel image for a magic number. You
provide it as a static in a special linker section:

```rust
const MULTIBOOT_MAGIC: u32    = 0x1BAD_B002;
const MULTIBOOT_FLAGS: u32    = 0x0000_0003;  // page-align + memory info
const MULTIBOOT_CHECKSUM: u32 = 0u32.wrapping_sub(MAGIC + FLAGS);

#[used]
#[link_section = ".multiboot"]
static MULTIBOOT_HEADER: [u32; 3] = [MULTIBOOT_MAGIC, MULTIBOOT_FLAGS, MULTIBOOT_CHECKSUM];
```

`#[used]` prevents the compiler from optimizing away a static that no code
reads. `#[link_section = ".multiboot"]` places it in a named section that
the linker script (next section) puts at the front of the image.

The flags value (`0x0000_0003`) sets two bits: bit 0 requests page-aligned
loading, bit 1 requests the memory map from GRUB.

## 5. Linker Script

The linker decides where code and data sit in the final binary. For a kernel,
you need explicit control over this layout.

**Essential pieces:**

```ld
ENTRY(_start)      // ELF entry point — where GRUB jumps
. = 1M;            // place everything at or above 1 MiB
KEEP(*(.multiboot)) // keep the Multiboot header even though no code references it
```

### 5.1 Full Walkthrough

```ld
ENTRY(_start)
```

Sets the ELF entry point to the `_start` label. GRUB reads `e_entry` from the
ELF header and jumps there. If this is wrong, the CPU executes garbage.

```ld
SECTIONS
{
    . = 1M;
```

The location counter (`.`) is set to 1,048,576 (0x100000). Every section that
follows starts at or after this address. The first megabyte of x86 physical
memory is reserved for the interrupt vector table, BIOS data area, VGA
framebuffer, and option ROMs. 1 MiB and above is free for your kernel.

```ld
    .boot :
    {
        KEEP(*(.multiboot))
    }
```

The `.boot` output section contains only the Multiboot header. `*(.multiboot)`
gathers every object file's `.multiboot` input section. Making this the first
output section guarantees the header is within the first 8 KiB of the image.

**Why `KEEP`?** The linker's garbage collector (`--gc-sections`) would discard
`.multiboot` because no code references those three `u32` values. Their only
purpose is for GRUB to find them. `KEEP` blocks garbage collection.

> **Common pitfall:** Without `KEEP`, GRUB finds no Multiboot magic number
> and silently refuses to boot. The `grub-file --is-x86-multiboot` check
> will fail. Always verify with `make verify` after editing the linker
> script or the header.

```ld
    .text :
    {
        *(.text .text.*)
    }
```

Executable code. `_start`, `kernel_main`, and every function live here.
`*(.text .text.*)` matches all `.text` sections plus subsections like
`.text.main` that the compiler generates.

```ld
    .rodata :
    {
        *(.rodata .rodata.*)
    }
```

Read-only data: string literals, constant tables, `static` values declared
without `mut`. Kept separate from `.text` so page permissions can differ when
you implement paging later.

```ld
    .data :
    {
        *(.data .data.*)
    }
```

Initialized read-write data. In a minimal kernel this section is usually
empty — `#![no_std]` code rarely produces initialized mutable statics.
Declare it anyway so when you add one, it lands in the right place.

```ld
    .bss :
    {
        *(COMMON)
        *(.bss .bss.*)
    }
}
```

Zero-initialized data. Contains your kernel stack (reserved with `.skip 16384`
in the assembly stub). `*(COMMON)` catches tentative definitions from
compiler-generated symbols.

**Why is `.bss` last?** It's the only section whose contents are *not stored
in the ELF file*. The bootloader reads its VMA and size, allocates memory, and
fills it with zeros. By placing it last, the loader extends the final loaded
segment with zeroed pages — no need to skip past file data.

### 5.2 Observed Memory Layout

From `objdump -h` on the compiled kernel:

| Section   | Start         | End           | Size    | Contents                     |
|-----------|---------------|---------------|---------|------------------------------|
| `.boot`   | `0x00100000`  | `0x0010000B`  | 12 B    | Multiboot header             |
| `.text`   | `0x00100010`  | `0x0010DBA2`  | ~56 KiB | All code                     |
| `.rodata` | `0x0010DBA4`  | `0x00110635`  | ~11 KiB | String literals, constants   |
| `.bss`    | `0x00110640`  | `0x0011463F`  | 16 KiB  | Stack (zeroed)               |

`.data` is absent — no initialized mutable statics. The kernel's total
footprint is ~82 KiB. `KERNEL_END = .;` after `.bss` marks address
`0x00114640` as the first free byte, which the physical memory manager
uses as its starting point.

## 6. CPU Entry and Stack

GRUB enters your kernel in 32-bit protected mode. Rust functions need a valid
stack, so you write a small assembly stub that sets one up before calling
Rust:

```asm
.section .text
.global _start
_start:
    lea esp, [stack_top]
    push ebx            ; multiboot info pointer
    push eax            ; multiboot magic
    call kernel_main

1:  hlt
    jmp 1b

.section .bss
.align 16
stack_bottom:
    .skip 16384
stack_top:
```

`_start` is the first code GRUB executes. It sets the stack pointer to the
top of a 16 KiB zeroed region, pushes the two values GRUB passes in `eax`
(magic) and `ebx` (multiboot info pointer), and calls `kernel_main`. The
defensive `hlt` loop after `call` catches the impossible case where
`kernel_main` accidentally returns.

> **Sharp edge — `lea` vs `mov`:** The instruction must be
> `lea esp, [stack_top]`, **not** `mov esp, stack_top`. In LLVM's Intel
> syntax, `mov esp, stack_top` is assembled as `mov esp, [stack_top]` — a
> memory dereference that loads garbage into the stack pointer. The kernel
> crashes before reaching Rust. Always verify with `objdump -d` that the
> disassembly shows `lea` (opcode `8d`), not `mov` (opcode `8b`).

## 7. VGA Text Output

Before graphics drivers exist, the simplest way to display text is VGA text
mode. The buffer is at physical address `0xb8000`. Each character cell is two
bytes: the ASCII byte followed by a color attribute byte.

Write with `core::ptr::write_volatile` — the compiler doesn't know anything
is reading from `0xb8000`, so volatile writes prevent it from optimizing them
away.

Implement `core::fmt::Write` for your VGA writer struct to unlock `write!`
and `writeln!` macros without the standard library.

## 8. Serial Output

QEMU can redirect a PC serial port to a host file or terminal. Serial is a
more reliable debugging channel than VGA screenshots, especially in headless
mode.

Initialize COM1 at I/O port `0x3f8`. Unlike VGA (which is memory-mapped),
serial ports use x86 I/O instructions:

```asm
out dx, al    ; write byte to port
in  al, dx    ; read byte from port
```

Wrap these in `unsafe fn outb(port: u16, value: u8)` and `unsafe fn inb(port:
u16) -> u8` using inline assembly. Implement `core::fmt::Write` for your
serial struct the same way as VGA.

The UART must be initialized before writing — set the baud rate divisor,
configure 8N1 framing, and enable the FIFO. The OSDev Wiki has a reference
16550 init sequence.

## 9. Halting

After boot messages, the kernel should stop cleanly:

```rust
loop {
    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)); }
}
```

`hlt` tells the CPU to sleep until the next interrupt. With no interrupt
handlers yet, this is a permanent stop.

## 10. Build System

Three layers:

```sh
make kernel   # cargo build → target/i686-kernel/debug/rust_kernel
make iso      # copy ELF to iso/boot/kernel.bin, run grub-mkrescue
make run      # boot the ISO in QEMU with serial output
```

**Custom target spec** (`targets/i686-kernel.json`): The kernel needs a
32-bit i686 target with no OS. This target isn't shipped as a prebuilt Rust
target, so Cargo must build `core` from source. Requires nightly Rust and
`rust-src`:

```sh
rustup toolchain install nightly --component rust-src
```

The target spec sets: 32-bit little-endian x86, `"os": "none"`, panic abort,
and the correct LLVM `data-layout`.

**Makefile variables** point to explicit tool paths so the user's shell
`PATH` doesn't need customization. On macOS with Homebrew, the cross-compiled
GRUB tools are named `i686-elf-grub-mkrescue`.

## 11. Physical Memory Management

Once the kernel boots, it needs to know which RAM it can use.

### 11.1 Reading the Multiboot Memory Map

GRUB passes a pointer to a multiboot info structure in `ebx` (received as the
second argument to `kernel_main`). The relevant fields:

- **`flags`** (offset 0): bit 6 (0x40) indicates the memory map is present.
- **`mmap_length`** (offset 44): total bytes of the memory map.
- **`mmap_addr`** (offset 48): pointer to an array of memory map entries.

Each entry is a 24-byte packed struct:

| Offset | Field       | Type  | Meaning                        |
|--------|-------------|-------|--------------------------------|
| 0      | `size`      | `u32` | Entry size minus this field (20) |
| 4      | `base_addr` | `u64` | Physical start address         |
| 12     | `length`    | `u64` | Region size in bytes           |
| 20     | `typ`       | `u32` | 1 = usable RAM                 |

Parse these entries, filter for `typ == 1` and `base_addr < 4 GiB` and
`base_addr >= 1 MiB`, and collect the results.

### 11.2 Kernel End Symbol

Add `KERNEL_END = .;` after `.bss` in the linker script. This creates a
symbol at the first byte past your kernel's memory footprint. Declare it in
Rust as:

```rust
extern "C" {
    static KERNEL_END: u8;
}
```

The address `&KERNEL_END as *const u8 as u32` is where free RAM begins from
the allocator's perspective.

### 11.3 Page Frame Allocator

With the memory map and kernel end address, build a bitmap allocator that
tracks which 4 KiB pages are free. A bitmap works well: one bit per page,
where `0` means free and `1` means allocated.

Operations:
- **`init`**: Walk the memory map and mark all pages in free regions as
  free (0), everything else as used (1), including the kernel's own memory.
- **`alloc_page() -> Option<u32>`**: Find the first free bit, set it to 1,
  return the physical address.
- **`free_page(addr: u32)`**: Compute the bit index, set to 0.

Test by allocating a few pages and printing their addresses — they should be
sequential and start near 2–3 MiB.

## 12. Debugging

### 12.1 Verification Commands

```sh
make verify     # validates Multiboot header and shows _start disassembly

# Manual equivalents:
i686-elf-grub-file --is-x86-multiboot target/i686-kernel/debug/rust_kernel
objdump -h target/i686-kernel/debug/rust_kernel
objdump -d target/i686-kernel/debug/rust_kernel | grep -A 5 '<_start>:'
```

### 12.2 Boot With Serial Output

```sh
make run
# Equivalent to:
# qemu-system-i386 -cdrom target/rust_kernel.iso -boot d -serial stdio
```

For headless operation:

```sh
make run-headless
# Serial logged to target/serial.log, QEMU monitor on stdio
```

In the QEMU monitor (`(qemu)` prompt):

```text
screendump target/screen.ppm   # capture framebuffer
quit                           # exit QEMU
```

Convert screenshots on macOS:

```sh
sips -s format png target/screen.ppm --out target/screen.png
```

### 12.3 Troubleshooting Checklist

If serial output is empty:

1. **Rebuild** — `make clean && make kernel && make iso`.
2. **Validate** — `make verify`. Multiboot header must pass.
3. **Disassemble** — confirm `_start` uses `lea` (opcode `8d`), not `mov`
   (opcode `8b`).
4. **QEMU binary** — prefer `qemu-system-i386` for 32-bit kernels.
5. **Serial flag** — `-serial file:...` must come *after* `-cdrom` and
   `-boot d`.
6. **GRUB debug** — add `echo` commands to `grub.cfg` and boot with a
   visible display to confirm GRUB reaches `boot`.

## 13. Next Features

Each feature below builds on the previous. The pattern: create a module in
`src/`, call `init()` from `kernel_main`, verify with serial output.

### 13.1 Interrupt Descriptor Table (IDT)

The IDT tells the CPU which function to call for each interrupt or exception.
Without it, any fault (division by zero, page fault, timer) triple-faults the
CPU and reboots.

**What to build:**
- A 256-entry table of 8-byte gate descriptors (`#[repr(C, align(16))]`).
- Load it with the `lidt` instruction (operand is a 6-byte struct with a
  `u16` limit and a `*const` base pointer).
- Each entry is an interrupt gate (type `0x8E`) with code segment selector
  `0x08` (GRUB's kernel code segment) and a pointer to a handler function.
- Start with handlers for exceptions 0–31 that print the vector number and
  halt.
- Test with `unsafe { asm!("int3") }` to trigger a breakpoint — your handler
  should run.

### 13.2 Programmable Interrupt Controller (PIC)

The PIC routes hardware IRQs to CPU interrupt vectors. Its default vector
range (0–15) overlaps with x86 exceptions, so you must remap it.

**What to build:**
- Remap master PIC to vectors 32–39, slave to 40–47 via four ICW
  (Initialization Command Word) `outb` sequences.
- Master PIC ports: 0x20 (command), 0x21 (data). Slave: 0xA0, 0xA1.
- Mask all interrupts initially, then unmask specific ones (timer = IRQ0,
  keyboard = IRQ1).
- After every IRQ handler, send EOI: `outb(0x20, 0x20)` to master, plus
  `outb(0xA0, 0x20)` for IRQs 8–15.

### 13.3 Keyboard Interrupt

Once the PIC is remapped and IDT entry exists for IRQ1 (vector 33), read
scancodes from port 0x60.

**What to build:**
- Register an interrupt handler for vector 33.
- In the handler: read `inb(0x60)`, send EOI, print the scancode via serial.
- Map scancodes to ASCII for a basic keyboard driver.
- Test: type a key in QEMU and see the scancode appear.

### 13.4 Paging

Paging maps virtual addresses to physical addresses, enabling memory
protection, copy-on-write, and swapping.

**What to build:**
- A page directory (4 KiB-aligned, 1024 `u32` entries) and at least one page
  table.
- Each PDE points to a page table; each PTE maps a 4 KiB page. Use 4 MiB
  pages initially for simplicity (set bit 7, Page Size, in PDEs).
- Load the physical address of the page directory into `cr3`.
- Set bit 31 (PG) and bit 0 (PE) in `cr0` to enable paging.
- Identity-map the kernel's memory (virtual == physical) so it keeps running
  after the switch.
- Uses inline assembly: `mov cr3, eax` and `mov cr0, eax`.

### 13.5 Heap Allocation

With paging you know which physical memory is free. A heap allocator lets you
use `Box`, `Vec`, and other dynamic types.

**What to build:**
- Start with a bump allocator: a `next` pointer that advances on each
  allocation, never freeing. ~30 lines of Rust.
- Implement `GlobalAlloc` trait and mark with `#[global_allocator]`.
- Requires nightly Rust and `extern crate alloc;`.
- Upgrade to a free-list or slab allocator later for deallocation support.

### 13.6 Basic Scheduler

A scheduler lets multiple tasks share the CPU.

**What to build:**
- A task struct with a saved stack pointer and state.
- Cooperative multitasking: each task voluntarily yields. Save registers to
  the current task's stack, switch to the next task's stack, restore its
  registers, return.
- Later, use the timer interrupt (IRQ0, vector 32) for preemptive scheduling.
- Core context switch is ~50 lines of assembly.
- Test with two hardcoded tasks printing alternating messages.

## 14. The Full Roadmap

If you were starting from nothing — no GRUB, no Rust runtime, just a blank
file — here's the order to build everything:

**Phase 1 — Bare Metal Boot**
1. Freestanding binary: `#![no_std]`, `#![no_main]`, panic handler.
2. Linker script: place code at 1 MiB, define sections.
3. Multiboot header: magic + flags + checksum in the first 8 KiB.
4. Assembly stub: set up a stack, call Rust's entry point.
5. Serial output: initialize COM1, write bytes. Confirm you're alive.
6. VGA output: write to `0xb8000` for visual confirmation.

**Phase 2 — CPU Control**
7. GDT: set up code/data segments (GRUB leaves a usable one; reload for
   practice).
8. IDT: 256 interrupt gates, exception handlers for vectors 0–31.
9. PIC remapping: remap to vectors 32–47, enable timer IRQ.
10. Timer: configure PIT or APIC for a periodic interrupt — your heartbeat.

**Phase 3 — Memory**
11. Physical memory map: parse GRUB's multiboot info to find free RAM.
12. Page frame allocator: bitmap tracking free 4 KiB pages.
13. Paging: set up page tables, enable, identity-map kernel space.
14. Heap allocator: bump allocator, then free-list. `Box` and `Vec` work.

**Phase 4 — User Interaction**
15. Keyboard driver: scancode → key events.
16. Simple shell: read input, parse commands, print output.

**Phase 5 — Processes**
17. Multitasking: cooperative yielding, then preemptive via timer IRQ.
18. System calls: `int 0x80` or `sysenter`.
19. User mode: ring 3 code segments, TSS for stack switching.
20. ELF loader: parse, map segments, jump to entry point.

**Phase 6 — Polish**
21. Filesystem: in-memory FS or FAT32 driver.
22. Syscall table: organized dispatch.
23. Drivers: ATA disk, networking, graphics.

**Key principle:** at every phase, add the smallest thing that proves you got
it right. Write to serial before VGA. Handle one exception before all 32.
Map one page before the full address space. Boot in QEMU after every change.
If something breaks, you know exactly which 5 lines caused it.
