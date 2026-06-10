# ross
a very basic x86 kernel

## Quick start
Install rustup and the nightly toolchain, if you don't have them already:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install nightly --component rust-src
```

You also need grub and qemu x86.

arch:
```sh
sudo pacman -S grub xorriso qemu-system-x86
```

debian/ubuntu and derivatives:
```sh
sudo apt install grub-pc-bin xorriso qemu-system-x86
```

macos:
```sh
brew install i686-elf-grub qemu
```


Then just run the kernel with `make run`.

## Techy details

Current features:
- boots from grub with the multiboot spec
- serial i/o 
- basic page allocation/freeing
