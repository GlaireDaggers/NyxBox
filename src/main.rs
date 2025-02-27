use std::sync::{Arc, RwLock};

use clock::Clock;
use machine::Machine;
use mem::{Memory, BOOT_ROM_BEGIN, CLOCK_BEGIN, MAIN_RAM_BEGIN};
use sdl3::{event::Event, gpu::{ColorTargetInfo, Device, LoadOp, ShaderFormat, StoreOp}, pixels::Color};
use unicorn_engine::Permission;
use vdp::VDP;

extern crate sdl3;
extern crate unicorn_engine;
extern crate rsevents;

mod vdp;
mod mem;
mod peripheral;
mod machine;

mod clock;

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

    let mut mem = Memory::new();

    // https://shell-storm.org/online/Online-Assembler-and-Disassembler
    /*
    ldr r0,=0x0         ; disable RTC
    ldr r1,=0x8000000
    str r0, [r1]

    ldr r0,=12345       ; set clock
    ldr r1,=0x8000004
    str r0, [r1]

    ldr r0,=0x1         ; enable RTC
    ldr r1,=0x8000000
    str r0, [r1]

    ldr r1,=0x8000004

    my_program:
        wfi
        swi 0
        ldr r0, [r1]    ; read clock
        b my_program
     */
    let test_program: &[u8] = &[
        0x30, 0x00, 0x9f, 0xe5, 0x30, 0x10, 0x9f, 0xe5, 
        0x00, 0x00, 0x81, 0xe5, 0x2c, 0x00, 0x9f, 0xe5, 
        0x2c, 0x10, 0x9f, 0xe5, 0x00, 0x00, 0x81, 0xe5, 
        0x28, 0x00, 0x9f, 0xe5, 0x28, 0x10, 0x9f, 0xe5, 
        0x00, 0x00, 0x81, 0xe5, 0x24, 0x10, 0x9f, 0xe5, 
        0x03, 0xf0, 0x20, 0xe3, 0x00, 0x00, 0x00, 0xef, 
        0x00, 0x00, 0x91, 0xe5, 0xfb, 0xff, 0xff, 0xea, 
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 
        0x39, 0x30, 0x00, 0x00, 0x04, 0x00, 0x00, 0x08, 
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 
        0x04, 0x00, 0x00, 0x08,
    ];
    mem.boot_rom[0..test_program.len()].copy_from_slice(test_program);

    let mut machine = Machine::new();

    // map system memory
    machine.map_memory(&mut mem.boot_rom, BOOT_ROM_BEGIN as u32, Permission::READ | Permission::EXEC);
    machine.map_memory(&mut mem.main_ram, MAIN_RAM_BEGIN as u32, Permission::ALL);

    let clock = Arc::new(RwLock::new(Clock::new()));
    machine.map_peripheral(clock.clone(), CLOCK_BEGIN as u32, 4096);

    let mut vdp = VDP::new(&graphics_device);

    let cmd_buffer = graphics_device.acquire_command_buffer().unwrap();
    {
        // test: upload some vertex data into VRAM
        vdp.upload(&[
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
        vdp.upload(&[
            0x00000000,     // write internal register (FBDIM)
            0x01E00280,     // - value (640x480)
            0x00000100,     // write internal register (FBADDR)
            0x00000400,     // - value (0x400)
            0x00000102,     // draw triangle list (primitive count: 1)
            0x00000000,     // - address
            0xAABBCCFF,     // end of queue (token: 0xAABBCC)
        ], 64, &graphics_device, &cmd_buffer);

        // test: add command to queue
        vdp.set_reg(vdp::REG_CMDPORT, 64);
    }
    cmd_buffer.submit().unwrap();

    // start running the CPU
    let run_ctx = machine.run();

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

        let mut cmd_buf = graphics_device.acquire_command_buffer().unwrap();

        while accum >= TIMESTEP {
            accum -= TIMESTEP;
            
            println!("FRAME: {}", frame);
            frame += 1;

            // update VDP
            vdp.tick(&graphics_device, &cmd_buf);

            // todo: actual interrupts
            run_ctx.raise_signal();
        }

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

    run_ctx.stop();
}