unsafe fn read_u32(addr: u32) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

#[repr(C, packed)]
struct MmapEntry {
    size: u32,
    base_addr: u64,
    length: u64,
    typ: u32,
}

pub struct MemoryRegions {
    pub count: usize,
    pub regions: [(u32, u32); 32],
}

pub fn parse_memory_map(info_addr: u32) -> MemoryRegions {
    let flags = unsafe { read_u32(info_addr + 0) };
    if (flags & 0x40) == 0 {
        return MemoryRegions { count: 0, regions: [(0, 0); 32] };
    }

    let mmap_length = unsafe { read_u32(info_addr + 44) };
    let mmap_addr = unsafe { read_u32(info_addr + 48) };

    let mut offset: u32 = 0;
    let mut result = MemoryRegions { count: 0, regions: [(0, 0); 32] };

    while offset < mmap_length && result.count < 32 {
        let entry_ptr = (mmap_addr + offset) as *const MmapEntry;
        let e = unsafe { core::ptr::read_volatile(entry_ptr) };

        if e.typ == 1 && e.base_addr < 0x1_0000_0000 && e.base_addr >= 0x10_0000 {
            result.regions[result.count] = (e.base_addr as u32, e.length as u32);
            result.count += 1;
        }
        
        offset += e.size + 4; 
    }

    result
}
