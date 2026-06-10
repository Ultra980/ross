use crate::multiboot::MemoryRegions;

extern "C" {
    static KERNEL_END: u8;
}

const PAGE_SIZE: u32 = 4096;
const FREE_MAP_SIZE: usize = 8192; // 256mb
const NUM_PAGES: usize = FREE_MAP_SIZE * 8;

static mut FREE_MAP: [u8; FREE_MAP_SIZE] = [0; FREE_MAP_SIZE];

pub fn free_ram_start() -> u32 { // returns the start of the usable region of RAM
    unsafe { &KERNEL_END as *const u8 as u32 }
}

fn bitmap_indices(idx: usize) -> (usize, usize) {
    (idx / 8, idx % 8)
}

fn bitmap_mark_used(idx: usize) {
    let (byte, bit) = bitmap_indices(idx);

    unsafe { FREE_MAP[byte] |= (1u8 << bit) };
}

fn bitmap_mark_free(idx: usize) {
    let (byte, bit) = bitmap_indices(idx);

    unsafe { FREE_MAP[byte] &= !(1u8 << bit) };
}

fn bitmap_test_free(idx: usize) -> bool {
    let (byte, bit) = bitmap_indices(idx);

    unsafe { (FREE_MAP[byte] & (1u8 << bit)) == 0u8 }
}

pub fn init_pages(regions: &MemoryRegions) {
    for bit in 0..(FREE_MAP_SIZE * 8) {
        bitmap_mark_used(bit);
    }

    let mem_start = free_ram_start();

    for reg in 0..regions.count {
        let (base, len) = regions.regions[reg];

        let reg_start = (base / PAGE_SIZE) as usize;
        let reg_end = ((base + len - 1) / PAGE_SIZE) as usize;
        for page in reg_start..=reg_end {
            if page as u32 * PAGE_SIZE >= mem_start {
                bitmap_mark_free(page);
            }
        } 
    }
}

pub fn alloc_page() -> Option<u32> {
    let start = (free_ram_start() / PAGE_SIZE) as usize;

    for page in start..NUM_PAGES {
        if bitmap_test_free(page) {
            bitmap_mark_used(page);
            return Some(page as u32 * PAGE_SIZE);
        }
    }
    None
}

pub fn free_page(addr: u32) {
    bitmap_mark_free((addr / PAGE_SIZE) as usize);
}
