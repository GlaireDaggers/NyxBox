// 4MiB boot ROM
pub const BOOT_ROM_SIZE: usize = 4 * 1024 * 1024;

// 16MiB main ram
pub const MAIN_RAM_SIZE: usize = 16 * 1024 * 1024;

pub const BOOT_ROM_BEGIN: usize = 0x0000000;
// pub const BOOT_ROM_END: usize = BOOT_ROM_BEGIN + (BOOT_ROM_SIZE - 1);

pub const MAIN_RAM_BEGIN: usize = 0x1000000;
// pub const MAIN_RAM_END: usize = MAIN_RAM_BEGIN + (MAIN_RAM_SIZE - 1);

pub const CLOCK_BEGIN: usize = 0x8000000;

pub struct Memory {
    pub boot_rom: Box<[u8]>,
    pub main_ram: Box<[u8]>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            boot_rom: vec![0;BOOT_ROM_SIZE].into_boxed_slice(),
            main_ram: vec![0;MAIN_RAM_SIZE].into_boxed_slice(),
        }
    }

    /*pub fn load_bootrom<T: Copy>(self: &Self, addr: u32) -> T {
        let ptr: *const u8 = &self.boot_rom[(addr as usize) % BOOT_ROM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { return *ptr_t; }
    }

    pub fn store_bootrom<T: Copy>(self: &mut Self, addr: u32, val: T) {
        let ptr: *mut u8 = &mut self.boot_rom[(addr as usize) % BOOT_ROM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { *ptr_t = val; }
    }

    pub fn load_main<T: Copy>(self: &Self, addr: u32) -> T {
        let ptr: *const u8 = &self.main_ram[(addr as usize) % MAIN_RAM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { return *ptr_t; }
    }

    pub fn store_main<T: Copy>(self: &mut Self, addr: u32, val: T) {
        let ptr: *mut u8 = &mut self.main_ram[(addr as usize) % MAIN_RAM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { *ptr_t = val; }
    }*/
}