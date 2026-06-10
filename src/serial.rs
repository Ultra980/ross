use core::fmt::{Write, Result};
use crate::io::{outb, inb};

pub struct SerialIO;

pub const COM1: u16 = 0x3f8;

impl SerialIO {
    pub fn init(&mut self) {
        unsafe {
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x80);
            outb(COM1, 0x03);
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x03);
            outb(COM1 + 2, 0xc7);
            outb(COM1 + 4, 0x0b);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        while (unsafe { inb(COM1 + 5) }) & 0x20 == 0 {}
        unsafe {
            outb(COM1, byte);
        }
    }

    fn read_byte_ready(&mut self) -> bool {
        (unsafe { inb(COM1 + 5) } & 0x01) != 0
    }

    pub fn read_byte(&mut self) -> u8 {
        while !self.read_byte_ready() {}
        unsafe { inb(COM1) }
    }

    fn read_byte_async(&mut self) -> Option<u8> {
        if self.read_byte_ready() {
            Some(unsafe { inb(COM1) })
        } else {
            None
        }
    }
}

impl Write for SerialIO {
    fn write_str(&mut self, s: &str) -> Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}
