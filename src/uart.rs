use std::{collections::VecDeque, io::Write};

use crate::peripheral::Peripheral;

pub const UART_MEM_SIZE: u32 = 4096;

pub struct UART<W: Write> {
    rx: VecDeque<u8>,
    tx: W
}

impl <W: Write> UART<W> {
    pub fn new(out_buffer: W) -> Self {
        Self {
            rx: VecDeque::new(),
            tx: out_buffer,
        }
    }

    pub fn push_input(self: &mut Self, input: &[u8]) {
        for i in input {
            self.rx.push_back(*i);
        }
    }
}

impl <W: Write> Peripheral for UART<W> {
    fn read(self: &mut Self, addr: u32) -> u32 {
        match addr {
            0x00 => {
                // STATUS
                return 2 |                                      // TX fifo empty
                    if self.rx.len() == 0 { 8 } else { 0 };    // RX fifo empty
            }
            0x02 => {
                // RX
                if let Some(v) = self.rx.pop_front() {
                    return v as u32;
                }
                else {
                    return 0;
                }
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

                // reset
                if (val & 1) != 0 {
                    self.rx.clear();
                    self.tx.flush().unwrap();
                }
            }
            0x01 => {
                // TX
                let b = (val & 0xFF) as u8;
                self.tx.write(&[b]).unwrap();
            }
            _ => {
            }
        }
    }
}