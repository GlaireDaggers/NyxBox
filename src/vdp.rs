use std::{collections::VecDeque, sync::{Arc, RwLock}};

use sdl3::gpu::{Buffer, BufferMemMap, BufferRegion, CommandBuffer, Device, TransferBuffer, TransferBufferLocation, TransferBufferUsage};

use crate::mem::Memory;

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
const INTERNALREG_VUSTRIDE: u32             = 1;
const INTERNALREG_VULAYOUT0: u32            = 2;
const INTERNALREG_VUCDATA0: u32             = 10;
const INTERNALREG_VUPROGADDR: u32           = 74;
const INTERNALREG_FOGENCOL: u32             = 75;
const INTERNALREG_FOGTBL0: u32              = 76;
const INTERNALREG_CLIPXY: u32               = 140;
const INTERNALREG_CLIPWH: u32               = 141;
const INTERNALREG_VPXY: u32                 = 142;
const INTERNALREG_VPWH: u32                 = 143;
const INTERNALREG_DEPTH: u32                = 144;
const INTERNALREG_BLEND: u32                = 145;
const INTERNALREG_CULL: u32                 = 146;
const INTERNALREG_TU0CONF: u32              = 147;
const INTERNALREG_TU1CONF: u32              = 148;
const INTERNALREG_TU0ADDR: u32              = 149;
const INTERNALREG_TU1ADDR: u32              = 150;
const INTERNALREG_TCOMBINE: u32             = 151;

const VTX_CACHE_SIZE: u32                   = 1024 * 1024 * 8;

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
    internal_reg: [u32;256],
    mem: Arc<RwLock<Memory>>,
    reset_state: bool,
    cmd_fifo: VecDeque<u32>,
    last_cmd_tok: VecDeque<u32>,
    cable_type: DisplayCable,
    display_enable: bool,
    display_interlace: bool,
    err_mode: ErrorMode,
    vtx_cache_in: Buffer,
    vtx_cache_out: Buffer,
    framebuffer: Buffer,
    depthbuffer: Buffer,
    tu0_cache: Buffer,
    tu1_cache: Buffer,
    vtx_transfer: TransferBuffer,
}

impl VDP {
    pub fn new(mem: Arc<RwLock<Memory>>, graphics_device: &Device) -> VDP {
        let vtx_cache_in = graphics_device.create_buffer()
            .with_size(VTX_CACHE_SIZE)
            .build()
            .unwrap();

        let vtx_cache_out = graphics_device.create_buffer()
            .with_size(VTX_CACHE_SIZE)
            .build()
            .unwrap();

        let vtx_transfer = graphics_device.create_transfer_buffer()
            .with_size(VTX_CACHE_SIZE)
            .with_usage(TransferBufferUsage::Upload)
            .build()
            .unwrap();

        // embedded framebuffer memory can hold up to 1024x1024 rgba32 image
        let framebuffer = graphics_device.create_buffer()
            .with_size(1024 * 1024 * 4)
            .build()
            .unwrap();

        // embedded depthbuffer memory can hold up to 1024x1024 f32 image
        let depthbuffer = graphics_device.create_buffer()
            .with_size(1024 * 1024 * 4)
            .build()
            .unwrap();

        // each TU cache is 2MiB - can hold up to 512x512 rgba32 mipmapped image, or 1024x1024 DBTC mipmapped image
        let tu0_cache = graphics_device.create_buffer()
            .with_size(1024 * 1024 * 2)
            .build()
            .unwrap();

        let tu1_cache = graphics_device.create_buffer()
            .with_size(1024 * 1024 * 2)
            .build()
            .unwrap();

        VDP {
            internal_reg: [0;256],
            mem,
            reset_state: false,
            cmd_fifo: VecDeque::new(),
            last_cmd_tok: VecDeque::new(),
            cable_type: DisplayCable::VGA,
            display_enable: false,
            display_interlace: false,
            err_mode: ErrorMode::None,
            vtx_cache_in,
            vtx_cache_out,
            framebuffer,
            depthbuffer,
            tu0_cache,
            tu1_cache,
            vtx_transfer,
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

    pub fn tick(self: &mut Self, graphics_device: &Device) {
        let cmd_buffer = graphics_device.acquire_command_buffer().unwrap();

        // execute commands
        let cmds = self.cmd_fifo.drain(0..).collect::<Vec<u32>>();
        for cmd_addr in cmds {
            self.exec_cmd_queue(cmd_addr, graphics_device, &cmd_buffer);
        }

        cmd_buffer.submit().unwrap();
    }

    fn reset(self: &mut Self) {
        for r in &mut self.internal_reg {
            *r = 0;
        }
        self.cmd_fifo.clear();
        self.last_cmd_tok.clear();
        self.display_enable = false;
        self.display_interlace = false;
        self.reset_state = false;
        self.err_mode = ErrorMode::None;
    }

    fn check_addr(self: &mut Self, addr: u32) -> bool {
        if addr % 4 != 0 {
            self.err_mode = ErrorMode::AddressError;
            return false;
        }

        return true;
    }

    fn load_word(mem: &Memory, addr: &mut u32) -> u32 {
        let word = mem.load::<u32>(*addr);
        *addr += 4;
        return word;
    }

    fn load_single(mem: &Memory, addr: &mut u32) -> f32 {
        let word = mem.load::<f32>(*addr);
        *addr += 4;
        return word;
    }

    fn exec_cmd_queue(self: &mut Self, mut addr: u32, gfx_device: &Device, cmd_buffer: &CommandBuffer) {
        if !self.check_addr(addr) {
            return;
        }

        let mem = self.mem.read().unwrap();

        loop {
            let hdr = Self::load_word(&mem, &mut addr);
            let op = hdr & 0xFF;

            match op {
                // write internal register
                0 => {
                    let register_idx = (hdr >> 8) & 0xFF;
                    let register_val = Self::load_word(&mem, &mut addr);
                    self.internal_reg[register_idx as usize] = register_val;
                }
                // draw list
                1 => {
                    let _topology = (hdr >> 8) & 3;
                    let count = hdr >> 10;
                    let ptr = Self::load_word(&mem, &mut addr) as usize;

                    // note: cannot submit more than 8MiB of data at a time
                    let stride = self.internal_reg[INTERNALREG_VUSTRIDE as usize];
                    let num_bytes = (stride * count).min(VTX_CACHE_SIZE) as usize;

                    let src_slice = &mem.main_ram[ptr..][..num_bytes];

                    // copy vertex data to input cache
                    let mut map = self.vtx_transfer.map::<u8>(gfx_device, true);
                    map.mem_mut().copy_from_slice(src_slice);
                    drop(map);

                    let copy_pass = gfx_device.begin_copy_pass(cmd_buffer).unwrap();
                    copy_pass.upload_to_gpu_buffer(TransferBufferLocation::new()
                        .with_transfer_buffer(&self.vtx_transfer)
                    , BufferRegion::new()
                        .with_buffer(&self.vtx_cache_in)
                        .with_size(num_bytes as u32)
                    , true);
                    gfx_device.end_copy_pass(copy_pass);

                    // transform vertex data into output cache
                }
                // clear color
                2 => {
                    let _color = Self::load_word(&mem, &mut addr);
                }
                // clear depth
                3 => {
                    let _depth = Self::load_word(&mem, &mut addr);
                }
                // swap buffers
                4 => {
                    let _copy = (hdr >> 8) & 1 != 0;
                    let _copy_target = Self::load_word(&mem, &mut addr);
                }
                // end of queue
                5 => {
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