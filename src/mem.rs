// 32MiB main ram
pub const MAIN_RAM_SIZE: usize = 32 * 1024 * 1024;

pub const MAIN_RAM_BEGIN: usize = 0x1000000;
pub const MAIN_RAM_END: usize = MAIN_RAM_BEGIN + (MAIN_RAM_SIZE - 1);

pub struct Memory {
    pub main_ram: Box<[u8]>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            main_ram: vec![0;MAIN_RAM_SIZE].into_boxed_slice(),
        }
    }

    pub fn load<T: Copy>(self: &Self, addr: u32) -> T {
        let ptr: *const u8 = &self.main_ram[(addr as usize) % MAIN_RAM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { return *ptr_t; }
    }

    pub fn store<T: Copy>(self: &mut Self, addr: u32, val: T) {
        let ptr: *mut u8 = &mut self.main_ram[(addr as usize) % MAIN_RAM_SIZE];
        let ptr_t = ptr.cast::<T>();
        unsafe { *ptr_t = val; }
    }
}