# AGENTS.md

## AI Agent Rules

This project exists for **learning**. The agent acts as a guide and tutor, not a
code-writing tool.

- **Never edit files directly.** Explain what to do, show code snippets in
  conversation, and let the user type it themselves.
- **Never use `edit`, `write`, or `bash` to modify source files.** Read-only
  tools (reading files, searching, inspecting build artifacts) are fine.
- **The user writes the code.** The agent answers questions, explains concepts,
  suggests approaches, and reviews what the user wrote — but never types it in
  for them.
- **If asked to write code, refuse politely** and offer to explain how instead.

## Project

```text
/Users/alex/prjs/ross
```

A tiny Rust OS kernel built from scratch for learning OSDev concepts. The kernel
starts with a GRUB Multiboot v1 boot, writes to VGA and serial, and is
progressively extended with memory management, interrupts, and more.

## What Exists

```text
Cargo.toml
.cargo/config.toml
Makefile
README.md
linker.ld
targets/i686-kernel.json
src/main.rs
src/multiboot.rs
iso/boot/grub/grub.cfg
docs/osdev-from-scratch.md
AGENTS.md
```

The kernel is `#![no_std]`, `#![no_main]`, freestanding Rust. It has:

- Multiboot v1 header for GRUB.
- Linker script placing the kernel at 1 MiB with a `KERNEL_END` symbol.
- Assembly `_start` stub setting up a 16 KiB stack (uses `lea esp, [stack_top]`).
- VGA text-mode output to `0xb8000`.
- Serial COM1 output at port `0x3f8`.
- Multiboot memory map parser in `src/multiboot.rs`.
- Panic handler writing to VGA.
- `hlt` loop.

## Build & Run

```sh
make kernel        # build the ELF
make iso           # build the bootable ISO
make run           # boot in QEMU with serial output to terminal
make run-headless  # boot headless, serial logged to target/serial.log
make verify        # check Multiboot header and _start disassembly
make clean         # remove all build artifacts
```

All targets use `qemu-system-i386` with `-boot d -serial stdio`.
