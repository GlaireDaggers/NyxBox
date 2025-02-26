use std::{collections::VecDeque, fs};

use sdl3::gpu::{Buffer, BufferMemMap, BufferRegion, BufferUsageFlags, CommandBuffer, ComputePipeline, Device, ShaderFormat, StorageBufferReadWriteBinding, TransferBuffer, TransferBufferLocation, TransferBufferUsage};

pub const REG_STATUS: usize         = 0;
pub const REG_CMDPORT: usize        = 1;
pub const REG_DISPLAYMODE: usize    = 2;

pub const STATUSBIT_RESET: u32              = 1;
pub const STATUSBIT_CMDFIFOEMPTY: u32       = 2;
pub const STATUSBIT_CMDFIFOFULL: u32        = 4;

pub const STATUSBIT_ERR_MASK: u32           = 0x18;
pub const STATUSBIT_ERR_ADDR: u32           = 0x8;
pub const STATUSBIT_ERR_CMD: u32            = 0x10;

pub const DISPLAYBIT_CABLE_MASK: u32        = 0b11;
pub const DISPLAYBIT_CABLE_VGA: u32         = 0;
pub const DISPLAYBIT_CABLE_COMPOSITE: u32   = 1;
pub const DISPLAYBIT_CABLE_SVIDEO: u32      = 2;
pub const DISPLAYBIT_CABLE_COMPONENT: u32   = 3;
pub const DISPLAYBIT_ENABLE: u32            = 4;
pub const DISPLAYBIT_INTERLACE: u32         = 8;

const INTERNALREG_FBDIM: u32                = 0;
const INTERNALREG_FBADDR: u32               = 1;
const INTERNALREG_DBADDR: u32               = 2;
const INTERNALREG_VUSTRIDE: u32             = 3;
const INTERNALREG_VULAYOUT0: u32            = 4;
const INTERNALREG_VUCDATA0: u32             = 12;
const INTERNALREG_VUPROGADDR: u32           = 76;
const INTERNALREG_FOGENCOL: u32             = 77;
const INTERNALREG_FOGTBL0: u32              = 78;
const INTERNALREG_CLIPXY: u32               = 142;
const INTERNALREG_CLIPWH: u32               = 143;
const INTERNALREG_VPXY: u32                 = 144;
const INTERNALREG_VPWH: u32                 = 145;
const INTERNALREG_DEPTH: u32                = 146;
const INTERNALREG_BLEND: u32                = 147;
const INTERNALREG_CULL: u32                 = 148;
const INTERNALREG_TUCONF: u32               = 149;
const INTERNALREG_TU0ADDR: u32              = 150;
const INTERNALREG_TU1ADDR: u32              = 151;
const INTERNALREG_TCOMBINE: u32             = 152;

const INTERNALREG_COUNT: usize              = 256;

// 8MiB VRAM
const VRAM_SIZE: u32 = 1024 * 1024 * 8;

#[repr(C)]
struct VertexUnitUBO {
    src_addr: u32,
    dst_addr: u32,
}

#[repr(C)]
struct DrawTriListUBO {
    addr: u32,
}

pub enum ErrorMode {
    None,
    AddressError,
    CmdError,
}

pub enum DisplayCable {
    VGA,
    Composite,
    SVideo,
    Component
}

pub enum Topology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
}

pub enum VDPCommand {
    WriteInternalRegister { reg: usize, val: u32 },
    DrawList { topology: Topology, addr: u32 },
    ClearColor { color: u32 },
    ClearDepth { depth: f32 },
    SwapBuffers { copy_target: Option<u32> },
    EndOfQueue { token: u32 },
}

pub struct VDP {
    internal_reg: [u32;INTERNALREG_COUNT],
    reset_state: bool,
    cmd_fifo: VecDeque<u32>,
    last_cmd_tok: VecDeque<u32>,
    cable_type: DisplayCable,
    display_enable: bool,
    display_interlace: bool,
    err_mode: ErrorMode,
    vram: Buffer,
    vram_transfer: TransferBuffer,
    regmem: Buffer,
    regmem_transfer: TransferBuffer,
    regmem_dirty: bool,
    vu_pipeline: ComputePipeline,
    draw_tri_list_pipeline: ComputePipeline,
}

impl VDP {
    pub fn new(graphics_device: &Device) -> VDP {
        let vram = graphics_device.create_buffer()
            .with_size(VRAM_SIZE)
            .with_usage(BufferUsageFlags::ComputeStorageRead | BufferUsageFlags::ComputeStorageWrite)
            .build()
            .unwrap();

        let vram_transfer = graphics_device.create_transfer_buffer()
            .with_size(VRAM_SIZE)
            .with_usage(TransferBufferUsage::Upload)
            .build()
            .unwrap();

        let regmem = graphics_device.create_buffer()
            .with_size((INTERNALREG_COUNT * 4) as u32)
            .with_usage(sdl3::gpu::BufferUsageFlags::ComputeStorageRead)
            .build()
            .unwrap();

        let regmem_transfer = graphics_device.create_transfer_buffer()
            .with_size((INTERNALREG_COUNT * 4) as u32)
            .with_usage(sdl3::gpu::TransferBufferUsage::Upload)
            .build()
            .unwrap();

        // load compute shaders
        let vu_shader = fs::read("content/shaders/vu.spv").unwrap();
        let vu_pipeline = graphics_device.create_compute_pipeline()
            .with_code(ShaderFormat::SpirV, &vu_shader)
            .with_entrypoint("main")
            .with_readonly_storage_buffers(1)
            .with_readwrite_storage_buffers(1)
            .with_uniform_buffers(1)
            .with_thread_count(1, 1, 1)
            .build().unwrap();

        let draw_tri_list_shader = fs::read("content/shaders/draw_tri_list.spv").unwrap();
        let draw_tri_list_pipeline = graphics_device.create_compute_pipeline()
            .with_code(ShaderFormat::SpirV, &draw_tri_list_shader)
            .with_entrypoint("main")
            .with_readonly_storage_buffers(1)
            .with_readwrite_storage_buffers(1)
            .with_uniform_buffers(1)
            .with_thread_count(1, 1, 1)
            .build().unwrap();

        VDP {
            internal_reg: [0;256],
            reset_state: false,
            cmd_fifo: VecDeque::new(),
            last_cmd_tok: VecDeque::new(),
            cable_type: DisplayCable::VGA,
            display_enable: false,
            display_interlace: false,
            err_mode: ErrorMode::None,
            vram,
            vram_transfer,
            regmem,
            regmem_transfer,
            regmem_dirty: true,
            vu_pipeline,
            draw_tri_list_pipeline,
        }
    }

    pub fn set_cable(self: &mut Self, cable: DisplayCable) {
        self.cable_type = cable;
    }

    pub fn get_reg(self: &mut Self, reg: usize) -> u32 {
        if reg == REG_STATUS {
            return
                if self.reset_state { STATUSBIT_RESET } else { 0 } |
                if self.cmd_fifo.len() == 0 { STATUSBIT_CMDFIFOEMPTY } else { 0 } |
                match self.err_mode {
                    ErrorMode::None => 0,
                    ErrorMode::AddressError => STATUSBIT_ERR_ADDR,
                    ErrorMode::CmdError => STATUSBIT_ERR_CMD,
                };
        }
        else if reg == REG_CMDPORT {
            return self.last_cmd_tok.pop_front().unwrap_or(0);
        }
        else if reg == REG_DISPLAYMODE {
            return
                match self.cable_type {
                    DisplayCable::VGA => DISPLAYBIT_CABLE_VGA,
                    DisplayCable::Composite => DISPLAYBIT_CABLE_COMPOSITE,
                    DisplayCable::SVideo => DISPLAYBIT_CABLE_SVIDEO,
                    DisplayCable::Component => DISPLAYBIT_CABLE_COMPONENT
                } |
                if self.display_enable { DISPLAYBIT_ENABLE } else { 0 } |
                if self.display_interlace { DISPLAYBIT_INTERLACE } else { 0 };
        }
        else {
            return 0;
        }
    }

    pub fn set_reg(self: &mut Self, reg: usize, value: u32) {
        if reg == REG_STATUS {
            if value & STATUSBIT_RESET == 0 {
                self.reset_state = true;
            }
        }
        else if reg == REG_CMDPORT {
            // value is address of command queue in main RAM
            self.cmd_fifo.push_back(value);
        }
        else if reg == REG_DISPLAYMODE {
            self.display_enable = (value & DISPLAYBIT_ENABLE) != 0;
            self.display_interlace = (value & DISPLAYBIT_INTERLACE) != 0;
        }
    }

    pub fn tick(self: &mut Self, graphics_device: &Device, cmd_buffer: &CommandBuffer) {
        // execute commands
        let cmds = self.cmd_fifo.drain(0..).collect::<Vec<u32>>();
        for cmd_addr in cmds {
            self.exec_cmd_queue(cmd_addr, graphics_device, &cmd_buffer);
        }
    }

    pub fn upload(self: &mut Self, mem: &[u32], dst_addr: u32, gfx_device: &Device, cmd_buffer: &CommandBuffer) {
        let mut vram: BufferMemMap<'_, u32> = self.vram_transfer.map::<u32>(gfx_device, false);
        vram.mem_mut()[dst_addr as usize..][..mem.len()].copy_from_slice(mem);
        drop(vram);

        let copy_pass = gfx_device.begin_copy_pass(cmd_buffer).unwrap();
        copy_pass.upload_to_gpu_buffer(
        TransferBufferLocation::new()
            .with_transfer_buffer(&self.vram_transfer)
            .with_offset(dst_addr), 
        BufferRegion::new()
            .with_buffer(&self.vram)
            .with_offset(dst_addr)
            .with_size(mem.len() as u32),
        false);
        gfx_device.end_copy_pass(copy_pass);
    }

    fn reset(self: &mut Self) {
        for r in &mut self.internal_reg {
            *r = 0;
        }
        self.regmem_dirty = true;
        self.cmd_fifo.clear();
        self.last_cmd_tok.clear();
        self.display_enable = false;
        self.display_interlace = false;
        self.reset_state = false;
        self.err_mode = ErrorMode::None;
    }

    fn load_word(mem: &BufferMemMap<u32>, addr: &mut u32) -> u32 {
        let word = mem.mem()[*addr as usize];
        *addr += 1;
        return word;
    }

    fn load_single(mem: &BufferMemMap<u32>, addr: &mut u32) -> f32 {
        let word = mem.mem()[*addr as usize];
        *addr += 1;
        return f32::from_bits(word);
    }

    fn flush_regmem(regmem_transfer: &mut TransferBuffer, regmem: &Buffer, internal_reg: &[u32], gfx_device: &Device, cmd_buffer: &CommandBuffer, regmem_dirty: &mut bool) {
        if *regmem_dirty {
            let mut transfer = regmem_transfer.map::<u32>(gfx_device, true);
            transfer.mem_mut().copy_from_slice(internal_reg);
            drop(transfer);

            let copy_pass = gfx_device.begin_copy_pass(cmd_buffer).unwrap();
            copy_pass.upload_to_gpu_buffer(TransferBufferLocation::new().with_transfer_buffer(&regmem_transfer),
                BufferRegion::new().with_buffer(&regmem).with_size(regmem.len()), true);
            gfx_device.end_copy_pass(copy_pass);

            *regmem_dirty = false;
        }
    }

    fn exec_cmd_queue(self: &mut Self, mut addr: u32, gfx_device: &Device, cmd_buffer: &CommandBuffer) {
        // command buffers reside in VRAM - lucky for us, we basically maintain a full copy of the VRAM state in a transfer buffer
        let mem: BufferMemMap<'_, u32> = self.vram_transfer.map::<u32>(gfx_device, false);

        loop {
            let hdr = Self::load_word(&mem, &mut addr);
            let op = hdr & 0xFF;

            match op {
                // write internal register
                0 => {
                    let register_idx = (hdr >> 8) & 0xFF;
                    let register_val = Self::load_word(&mem, &mut addr);
                    self.internal_reg[register_idx as usize] = register_val;
                    self.regmem_dirty = true;
                }
                // process vertex list
                1 => {
                    let count = hdr >> 8;
                    let src_ptr = Self::load_word(&mem, &mut addr);
                    let dst_ptr = Self::load_word(&mem, &mut addr);

                    Self::flush_regmem(&mut self.regmem_transfer, &self.regmem, &self.internal_reg, gfx_device, cmd_buffer, &mut self.regmem_dirty);

                    let compute_pass = gfx_device.begin_compute_pass(cmd_buffer, &[], &[
                        StorageBufferReadWriteBinding::new().with_buffer(&self.vram).with_cycle(false)
                    ]).unwrap();
                    {
                        compute_pass.bind_compute_pipeline(&self.vu_pipeline);
                        compute_pass.bind_compute_storage_buffers(0, &[&self.regmem]);

                        let ubo = VertexUnitUBO {
                            src_addr: src_ptr,
                            dst_addr: dst_ptr
                        };
                        cmd_buffer.push_compute_uniform_data(0, &ubo);

                        compute_pass.dispatch(count, 1, 1);
                    }
                    gfx_device.end_compute_pass(compute_pass);
                }
                // draw triangle list
                2 => {
                    let count = hdr >> 8;
                    let src_ptr = Self::load_word(&mem, &mut addr);

                    Self::flush_regmem(&mut self.regmem_transfer, &self.regmem, &self.internal_reg, gfx_device, cmd_buffer, &mut self.regmem_dirty);

                    let compute_pass = gfx_device.begin_compute_pass(cmd_buffer, &[], &[
                        StorageBufferReadWriteBinding::new().with_buffer(&self.vram).with_cycle(false)
                    ]).unwrap();
                    {
                        compute_pass.bind_compute_pipeline(&self.draw_tri_list_pipeline);
                        compute_pass.bind_compute_storage_buffers(0, &[&self.regmem]);

                        let ubo = DrawTriListUBO {
                            addr: src_ptr
                        };
                        cmd_buffer.push_compute_uniform_data(0, &ubo);

                        compute_pass.dispatch(count, 1, 1);
                    }
                    gfx_device.end_compute_pass(compute_pass);
                }
                // draw triangle strip
                3 => {
                    let _count = hdr >> 8;
                    let _src_ptr = Self::load_word(&mem, &mut addr);
                }
                // draw line list
                4 => {
                    let _count = hdr >> 8;
                    let _src_ptr = Self::load_word(&mem, &mut addr);
                }
                // draw line strip
                5 => {
                    let _count = hdr >> 8;
                    let _src_ptr = Self::load_word(&mem, &mut addr);
                }
                // clear color
                6 => {
                    let _color = Self::load_word(&mem, &mut addr);
                }
                // clear depth
                7 => {
                    let _depth = Self::load_word(&mem, &mut addr);
                }
                // end of queue
                0xFF => {
                    let token = hdr >> 8;
                    self.last_cmd_tok.push_back(token);
                    return;
                }
                _ => {
                    self.err_mode = ErrorMode::CmdError;
                    return;
                }
            }
        }
    }
}