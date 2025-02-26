#version 450
layout (local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

#define REG_FBDIM                0
#define REG_FBADDR               1
#define REG_DBADDR               2
#define REG_VUSTRIDE             3
#define REG_VULAYOUT0            4
#define REG_VUCDATA0             12
#define REG_VUPROGADDR           76
#define REG_FOGENCOL             77
#define REG_FOGTBL0              78
#define REG_CLIPXY               142
#define REG_CLIPWH               143
#define REG_VPXY                 144
#define REG_VPWH                 145
#define REG_DEPTH                146
#define REG_BLEND                147
#define REG_CULL                 148
#define REG_TUCONF               149
#define REG_TU0ADDR              150
#define REG_TU1ADDR              151
#define REG_TCOMBINE             152

layout(std430, set = 0, binding = 0) readonly buffer Params {
    uint data[256];
} params;

layout(std430, set = 1, binding = 0) buffer VRAM {
    uint data[];
} vram;

layout(std140, set = 2, binding = 0) uniform UBO {
    uint addr;
} ubo;

struct VertexData {
    vec4 position;
    vec2 texcoord0;
    vec2 texcoord1;
    vec4 color0;
    vec4 color1;
};

vec2 loadVec2(uint addr) {
    uint val = vram.data[addr];
    vec2 outdata;
    outdata.x = uintBitsToFloat(vram.data[addr]);
    outdata.y = uintBitsToFloat(vram.data[addr + 1]);
    return outdata;
}

vec4 loadVec4(uint addr) {
    uint val = vram.data[addr];
    vec4 outdata;
    outdata.x = uintBitsToFloat(vram.data[addr]);
    outdata.y = uintBitsToFloat(vram.data[addr + 1]);
    outdata.z = uintBitsToFloat(vram.data[addr + 2]);
    outdata.w = uintBitsToFloat(vram.data[addr + 3]);
    return outdata;
}

vec4 loadUNorm4(uint addr) {
    uint val = vram.data[addr];
    vec4 outdata;
    outdata.x = float(bitfieldExtract(val, 0, 8)) / 255.0;
    outdata.y = float(bitfieldExtract(val, 8, 8)) / 255.0;
    outdata.z = float(bitfieldExtract(val, 16, 8)) / 255.0;
    outdata.w = float(bitfieldExtract(val, 24, 8)) / 255.0;
    return outdata;
}

VertexData loadVertex(uint addr) {
    VertexData vdata;
    vdata.position = loadVec4(addr);
    vdata.texcoord0 = loadVec2(addr + 4);
    vdata.texcoord1 = loadVec2(addr + 6);
    vdata.color0 = loadUNorm4(addr + 8);
    vdata.color1 = loadUNorm4(addr + 9);
    return vdata;
}

ivec2 ndcToScreen(vec2 ndc, vec4 vp) {
    ndc = ndc * 0.5 + 0.5;
    return ivec2((ndc * vp.zw) + vp.xy);
}

void setColor(uint fbAddr, uvec2 fbDim, uvec2 coord, uint col) {
    uint pxAddr = fbAddr + (coord.y * fbDim.x) + (coord.x);
    vram.data[pxAddr] = col;
}

void main() {
    // each work group processes one triangle of input

    // NOTE: vertex size is 10 words
    // - 4 words for position
    // - 4 words for texcoord 0 UV + texcoord 1 UV
    // - 2 words for col + ocol

    uint base_addr = ubo.addr + (gl_WorkGroupID.x * 30);
    VertexData v0 = loadVertex(base_addr);
    VertexData v1 = loadVertex(base_addr + 10);
    VertexData v2 = loadVertex(base_addr + 20);

    // clip space to NDC
    v0.position /= v0.position.w;
    v1.position /= v1.position.w;
    v2.position /= v2.position.w;

    // NDC to screen coords

    // get viewport
    uint vp_xy = params.data[REG_VPXY];
    uint vp_wh = params.data[REG_VPWH];
    vec4 vp = vec4(
        vp_xy & 0xFFFF,
        vp_xy >> 16,
        vp_wh & 0xFFFF,
        vp_wh >> 16
    );

    ivec2 v0_scr = ndcToScreen(v0.position.xy, vp);
    ivec2 v1_scr = ndcToScreen(v1.position.xy, vp);
    ivec2 v2_scr = ndcToScreen(v2.position.xy, vp);

    // plot points to framebuffer
    uint fb_addr = params.data[REG_FBADDR];
    uint fb_dim = params.data[REG_FBDIM];
    uvec2 fb_wh = uvec2(
        fb_dim & 0xFFFF,
        fb_dim >> 16
    );

    setColor(fb_addr, fb_wh, uvec2(v0_scr), 0xFF0000FF);
    setColor(fb_addr, fb_wh, uvec2(v1_scr), 0xFF00FF00);
    setColor(fb_addr, fb_wh, uvec2(v2_scr), 0xFFFF0000);
}