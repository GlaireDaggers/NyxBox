use std::{sync::{Arc, RwLock}, thread, u64};

use mem::{Memory, BOOT_ROM_BEGIN, BOOT_ROM_SIZE, MAIN_RAM_BEGIN, MAIN_RAM_END, MAIN_RAM_SIZE};
use rsevents::{AutoResetEvent, Awaitable, EventState};
use sdl3::{event::Event, gpu::{ColorTargetInfo, Device, LoadOp, ShaderFormat, StoreOp}, pixels::Color};
use unicorn_engine::{Mode, Permission, Unicorn};
use vdp::VDP;

extern crate sdl3;
extern crate unicorn_engine;
extern crate rsevents;

mod vdp;
mod mem;

// used to wake up CPU thread to handle interrupts
static CPU_SIGNAL: AutoResetEvent = AutoResetEvent::new(EventState::Unset);

pub fn main() {
    let sdl_context = sdl3::init().unwrap();
    let video_sys = sdl_context.video().unwrap();

    let window = video_sys.window("Hello, world!", 960, 720)
        .position_centered()
        .build()
        .unwrap();

    let graphics_device = Device::new(ShaderFormat::SpirV, false).unwrap()
        .with_window(&window).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mem = Arc::new(RwLock::new(Memory::new()));

    // https://shell-storm.org/online/Online-Assembler-and-Disassembler
    /*
    my_program:
        wfi
        swi 0
        b my_program
     */
    let test_program: &[u8] = &[
        0x03, 0xf0, 0x20, 0xe3,
        0x00, 0x00, 0x00, 0xef,
        0xfc, 0xff, 0xff, 0xea,
    ];
    mem.write().unwrap().boot_rom[0..test_program.len()].copy_from_slice(test_program);

    let mut _vdp = VDP::new(&graphics_device);

    let cmd_buffer = graphics_device.acquire_command_buffer().unwrap();
    {
        // test: upload some vertex data into VRAM
        _vdp.upload(&[
            // vertex 0
            (-1.0_f32).to_bits(),   // position
            (-1.0_f32).to_bits(),
            (0.0_f32).to_bits(),
            (1.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 0
            (0.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 1
            (0.0_f32).to_bits(),
            0xFF0000FF,             // color 0
            0,                      // color 1
            // vertex 1
            (1.0_f32).to_bits(),   // position
            (-1.0_f32).to_bits(),
            (0.0_f32).to_bits(),
            (1.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 0
            (0.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 1
            (0.0_f32).to_bits(),
            0xFF0000FF,             // color 0
            0,                      // color 1
            // vertex 2
            (0.0_f32).to_bits(),   // position
            (1.0_f32).to_bits(),
            (0.0_f32).to_bits(),
            (1.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 0
            (0.0_f32).to_bits(),
            (0.0_f32).to_bits(),    // texcoord 1
            (0.0_f32).to_bits(),
            0xFF0000FF,             // color 0
            0,                      // color 1
        ], 0, &graphics_device, &cmd_buffer);

        // test: upload a command buffer into VRAM
        _vdp.upload(&[
            0x00000000,     // write internal register (FBDIM)
            0x01E00280,     // - value (640x480)
            0x00000100,     // write internal register (FBADDR)
            0x00000400,     // - value (0x400)
            0x00000102,     // draw triangle list (primitive count: 1)
            0x00000000,     // - address
            0xAABBCCFF,     // end of queue (token: 0xAABBCC)
        ], 64, &graphics_device, &cmd_buffer);

        // test: add command to queue
        _vdp.set_reg(vdp::REG_CMDPORT, 64);

        // test: execute command queues
        _vdp.tick(&graphics_device, &cmd_buffer);
    }
    cmd_buffer.submit().unwrap();

    // spawn a thread for the CPU
    thread::spawn(move || {
        let mut uc_engine = Unicorn::new(unicorn_engine::Arch::ARM, Mode::ARM1176).unwrap();
        uc_engine.ctl_set_cpu_model(unicorn_engine::ArmCpuModel::UC_CPU_ARM_1176 as i32).unwrap();

        {
            let mut mem = mem.write().unwrap();
            unsafe {
                uc_engine.mem_map_ptr(BOOT_ROM_BEGIN as u64, BOOT_ROM_SIZE, Permission::READ | Permission::EXEC, mem.boot_rom.as_mut_ptr().cast()).unwrap();
                uc_engine.mem_map_ptr(MAIN_RAM_BEGIN as u64, MAIN_RAM_SIZE, Permission::ALL, mem.main_ram.as_mut_ptr().cast()).unwrap();
            }
        }

        // use to implement BIOS hooks
        uc_engine.add_intr_hook(|uc, intr| {
            if intr == 2 {
                // swi
                let addr = uc.pc_read().unwrap() - 4;
                let mut insr = [0;4];
                uc.mem_read(addr, &mut insr).unwrap();
                let swi_num = insr[0];

                println!("SWI: {}", swi_num);
            }
        }).unwrap();

        let mut pc = BOOT_ROM_BEGIN as u64;

        // run until WFI
        loop {
            uc_engine.emu_start(pc, u64::MAX, 0, 0).unwrap();
            pc = uc_engine.pc_read().unwrap();
            CPU_SIGNAL.wait();
        }
    });

    let mut frame = 0;
    let mut prev_tick = sdl3::timer::performance_counter();
    let mut accum = 0.0;

    const TIMESTEP: f64 = 1.0 / 60.0;

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    break 'running;
                }
                _ => {
                }
            }
        }

        let cur_tick = sdl3::timer::performance_counter();
        let delta_tick = cur_tick - prev_tick;
        let dt = delta_tick as f64 / sdl3::timer::performance_frequency() as f64;
        prev_tick = cur_tick;

        accum += dt;

        while accum >= TIMESTEP {
            accum -= TIMESTEP;
            
            println!("FRAME: {}", frame);
            frame += 1;

            // todo: vsync interrupt
            CPU_SIGNAL.set();
        }

        let mut cmd_buf = graphics_device.acquire_command_buffer().unwrap();
        if let Ok(swap_target) = cmd_buf.wait_and_acquire_swapchain_texture(&window) {
            let targets = [
                ColorTargetInfo::default()
                    .with_texture(&swap_target)
                    .with_clear_color(Color::RGB(0, 128, 255))
                    .with_load_op(LoadOp::Clear)
                    .with_store_op(StoreOp::Store)
            ];
            let render_pass = graphics_device.begin_render_pass(&cmd_buf, &targets, None).unwrap();
            graphics_device.end_render_pass(render_pass);
        }
        cmd_buf.submit().unwrap();

    }
}