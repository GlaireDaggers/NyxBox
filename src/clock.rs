use std::time::Instant;

use crate::peripheral::Peripheral;

pub const CLOCK_MEM_SIZE: u32 = 4096;

fn get_sdl_ctr() -> u64 {
    let ctr = sdl3::timer::performance_counter();
    let freq = sdl3::timer::performance_frequency();
    let ref_freq = 1000000;
    let div = freq / ref_freq;

    return ctr / div;
}

pub struct Clock {
    rtc_en: bool,
    ctr0_en: bool,
    ctr1_en: bool,
    ctr0_intr: bool,
    ctr1_intr: bool,
    ctr0_intr_p: u32,
    ctr1_intr_p: u32,
    ctr0: u64,
    ctr1: u64,
    dt_adjust: i64,
    time_start: Instant,
    ctr0_base: u64,
    ctr1_base: u64,
    timestamp: u32,
}

impl Clock {
    pub fn new() -> Self {
        let ctr_base = get_sdl_ctr();

        Self {
            rtc_en: false,
            ctr0_en: false,
            ctr1_en: false,
            ctr0_intr: false,
            ctr1_intr: false,
            ctr0_intr_p: 0,
            ctr1_intr_p: 0,
            ctr0: 0,
            ctr1: 0,
            ctr0_base: ctr_base,
            ctr1_base: ctr_base,
            dt_adjust: 0,
            time_start: Instant::now(),
            timestamp: 0,
        }
    }
}

impl Peripheral for Clock {
    fn read(self: &mut Self, addr: u32) -> u32 {
        match addr {
            0x00 => {
                // STATUS
                return
                    if self.rtc_en { 1 } else { 0 } |
                    if self.ctr0_en { 2 } else { 0 } |
                    if self.ctr1_en { 4 } else { 0 } |
                    if self.ctr0_intr { 32 } else { 0 } |
                    if self.ctr1_intr { 64 } else { 0 };
            }
            0x01 => {
                // DT
                if self.rtc_en {
                    let secs_since_startup = Instant::now().duration_since(self.time_start).as_secs() as i64;
                    self.timestamp = (secs_since_startup + self.dt_adjust) as u32;
                }
                else {
                    let secs_since_startup = Instant::now().duration_since(self.time_start).as_secs() as i64;
                    let desired_secs = self.timestamp as i64;
                    self.dt_adjust = desired_secs - secs_since_startup;
                }

                return self.timestamp;
            }
            0x02 => {
                // CTR0LO
                if self.ctr0_en {
                    self.ctr0 = get_sdl_ctr() - self.ctr0_base;
                }
                else {
                    self.ctr0_base = get_sdl_ctr() - self.ctr0;
                }
                return (self.ctr0 & 0xFFFFFFFF) as u32;
            }
            0x03 => {
                // CTR0HI
                return (self.ctr0 >> 32) as u32;
            }
            0x04 => {
                // CTR1LO
                if self.ctr1_en {
                    self.ctr1 = get_sdl_ctr() - self.ctr1_base;
                }
                else {
                    self.ctr1_base = get_sdl_ctr() - self.ctr1;
                }
                return (self.ctr1 & 0xFFFFFFFF) as u32;
            }
            0x05 => {
                // CTR1HI
                return (self.ctr1 >> 32) as u32;
            }
            0x06 => {
                // CTR0P
                return self.ctr0_intr_p;
            }
            0x07 => {
                // CTR1P
                return self.ctr1_intr_p;
            }
            _ => {
                return 0;
            }
        }
    }

    fn write(self: &mut Self, addr: u32, val: u32) {
        match addr {
            0x00 => {
                // STATUS
                self.rtc_en = (val & 1) != 0;
                self.ctr0_en = (val & 2) != 0;
                self.ctr1_en = (val & 4) != 0;
                self.ctr0_intr = (val & 32) != 0;
                self.ctr1_intr = (val & 64) != 0;

                let ctr_base = get_sdl_ctr();

                if (val & 8) != 0 {
                    // reset ctr0
                    self.ctr0_base = ctr_base;
                    self.ctr0 = 0;
                }

                if (val & 16) != 0 {
                    // reset ctr1
                    self.ctr1_base = ctr_base;
                    self.ctr1 = 0;
                }
                
                let secs_since_startup = Instant::now().duration_since(self.time_start).as_secs() as i64;
                self.timestamp = (secs_since_startup + self.dt_adjust) as u32;
            }
            0x01 => {
                // DT
                if !self.rtc_en {
                    let secs_since_startup = Instant::now().duration_since(self.time_start).as_secs() as i64;
                    let desired_secs = val as i64;
                    self.dt_adjust = desired_secs - secs_since_startup;
                }
            }
            0x06 => {
                // CTR0P
                self.ctr0_intr_p = val;
            }
            0x07 => {
                // CTR1P
                self.ctr1_intr_p = val;
            }
            _ => {
            }
        }
    }
}