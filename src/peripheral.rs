pub trait Peripheral {
    fn read(self: &mut Self, addr: u32) -> u32;
    fn write(self: &mut Self, addr: u32, val: u32);
}