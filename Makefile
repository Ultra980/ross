RUSTUP ?= rustup
CARGO  := $(RUSTUP) run nightly cargo
RUSTC  := $(shell $(RUSTUP) which rustc --toolchain nightly)
BUILD_STD := -Z json-target-spec -Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem
GRUB_MKRESCUE ?= i686-elf-grub-mkrescue
KERNEL := target/i686-kernel/debug/rust_kernel
ISO_ROOT := iso
ISO := target/rust_kernel.iso

.PHONY: all kernel iso run run-headless verify clean

all: kernel

kernel:
	RUSTC=$(RUSTC) $(CARGO) build $(BUILD_STD)

iso: kernel
	mkdir -p $(ISO_ROOT)/boot/grub target
	cp $(KERNEL) $(ISO_ROOT)/boot/kernel.bin
	$(GRUB_MKRESCUE) -o $(ISO) $(ISO_ROOT)

run: iso
	qemu-system-i386 -cdrom $(ISO) -boot d -serial stdio

run-headless: iso
	qemu-system-i386 -cdrom $(ISO) -boot d -serial file:target/serial.log -display none -monitor stdio

verify:
	@echo "checking multiboot header in kernel bin"
	@i686-elf-grub-file --is-x86-multiboot $(KERNEL) && echo "kernel ok" || echo "kernel failed"
	@echo "checking multiboot header in iso"
	@i686-elf-grub-file --is-x86-multiboot $(ISO_ROOT)/boot/kernel.bin && echo "iso ok" || echo "iso failed"

clean:
	cargo clean
	rm -f $(ISO_ROOT)/boot/kernel.bin $(ISO) target/serial.log
