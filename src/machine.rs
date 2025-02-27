use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, RwLock}, thread::{self, JoinHandle}};

use rsevents::{AutoResetEvent, Awaitable, EventState};
use unicorn_engine::{ffi::uc_handle, Mode, Permission, RegisterARM, Unicorn};

use crate::{mem::BOOT_ROM_BEGIN, peripheral::Peripheral};

pub struct Machine<'a> {
    cpu: Unicorn<'a, ()>,
}

pub struct MachineRunContext {
    join_handle: JoinHandle<()>,
    cpu_signal: Arc<AutoResetEvent>,
    stop_signal: Arc<AtomicBool>
}

impl <'a> Machine<'a> {
    pub fn new() -> Self {
        let mut cpu = Unicorn::new(unicorn_engine::Arch::ARM, Mode::ARM1176).unwrap();
        cpu.ctl_set_cpu_model(unicorn_engine::ArmCpuModel::UC_CPU_ARM_1176 as i32).unwrap();

        // use to implement BIOS hooks
        cpu.add_intr_hook(|uc, intr| {
            let r0 = uc.reg_read(RegisterARM::R0).unwrap();
            println!("R0: {}", r0);

            if intr == 2 {
                // swi
                let addr = uc.pc_read().unwrap() - 4;
                let mut insr = [0;4];
                uc.mem_read(addr, &mut insr).unwrap();
                let swi_num = insr[0];

                println!("SWI: {}", swi_num);
            }
        }).unwrap();

        Self {
            cpu: cpu,
        }
    }

    pub fn map_memory(self: &mut Self, mem: &'a mut [u8], start_addr: u32, permission: Permission) {
        unsafe {
            self.cpu.mem_map_ptr(start_addr as u64, mem.len(), permission, mem.as_mut_ptr().cast()).unwrap();
        }
    }

    pub fn map_peripheral<T>(self: &mut Self, device: Arc<RwLock<T>>, start_addr: u32, length: u32) where T : Peripheral + 'a {
        let rd_dev = device.clone();
        let wr_dev = device.clone();

        let rd = move |_uc: &mut Unicorn<'_, ()>, addr, _size| -> u64 {
            let local_addr = (addr & 0xFFFFFF) >> 2;
            let mut dev = rd_dev.write().unwrap();
            return dev.read(local_addr as u32) as u64;
        };

        let wr = move |_uc: &mut Unicorn<'_, ()>, addr, _size, value| {
            let local_addr = (addr & 0xFFFFFF) >> 2;
            let mut dev = wr_dev.write().unwrap();
            dev.write(local_addr as u32, value as u32);
        };

        // add read/write hooks
        self.cpu.mmio_map(start_addr as u64, length as usize, Some(rd), Some(wr)).unwrap();
    }

    pub fn run(self: &Self) -> MachineRunContext {
        // this is an awful no good very bad way to do this tbh
        // basically: turns underlying uc_handle into a usize, sends it to the thread, turns it back into a uc_handle, & makes a new Unicorn instance pointing to that handle

        // that said, the underlying API is *supposed* to be thread safe, so this should be OK

        let cpu_send = self.cpu.get_handle() as usize;
        let cpu_signal = Arc::new(AutoResetEvent::new(EventState::Unset));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let ret_cpu_signal = cpu_signal.clone();
        let ret_stop_signal = stop_signal.clone();

        let join_handle = thread::spawn(move || {
            let cpu_handle = cpu_send as uc_handle;
            let mut cpu = unsafe { Unicorn::from_handle(cpu_handle).unwrap() };

            let mut pc = BOOT_ROM_BEGIN as u64;

            // run until WFI, then wait for signal to resume
            loop {
                cpu.emu_start(pc, u64::MAX, 0, 0).unwrap();
                pc = cpu.pc_read().unwrap();
                cpu_signal.wait();

                if stop_signal.load(Ordering::Relaxed) {
                    break;
                }
            }
        });

        return MachineRunContext {
            join_handle,
            cpu_signal: ret_cpu_signal,
            stop_signal: ret_stop_signal
        };
    }
}

impl MachineRunContext {
    pub fn raise_signal(self: &Self) {
        self.cpu_signal.set();
    }

    pub fn stop(self: Self) {
        // set the stop signal, interrupt the CPU, & then wait for the thread to exit
        self.stop_signal.store(true, Ordering::Relaxed);
        self.cpu_signal.set();
        self.join_handle.join().unwrap();
    }
}