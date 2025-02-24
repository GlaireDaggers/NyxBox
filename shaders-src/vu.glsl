#version 450
layout (local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(std430, set = 0, binding = 0) readonly buffer InputCache {
    uint vtxdata[];
} inputCache;

layout(std430, set = 0, binding = 1) writeonly buffer OutputCache {
    uint vtxdata[];
} outputCache;

layout(std430, set = 0, binding = 2) buffer Params {
    uint stride;
    uint batch_size;
    uint slotlayout[8];
    vec4 cdata[16];
    uint program[64];
} params;

vec4 load_vtx_slot(uint base_addr, uint slotlayout) {
    // lower 3 bits of layout identifies slot type
    uint param_type = slotlayout & 7;

    // upper 28 bits of layout identifies offset from base address
    uint slot_addr = base_addr + (slotlayout >> 4);

    vec4 outdata = vec4(0.0, 0.0, 0.0, 0.0);

    switch (param_type) {
        case 0: {
            // FLOAT1
            outdata.x = uintBitsToFloat(inputCache.vtxdata[slot_addr]);
            break;
        }
        case 1: {
            // FLOAT2
            outdata.x = uintBitsToFloat(inputCache.vtxdata[slot_addr]);
            outdata.y = uintBitsToFloat(inputCache.vtxdata[slot_addr + 1]);
            break;
        }
        case 2: {
            // FLOAT3
            outdata.x = uintBitsToFloat(inputCache.vtxdata[slot_addr]);
            outdata.y = uintBitsToFloat(inputCache.vtxdata[slot_addr + 1]);
            outdata.z = uintBitsToFloat(inputCache.vtxdata[slot_addr + 2]);
            break;
        }
        case 3: {
            // FLOAT4
            outdata.x = uintBitsToFloat(inputCache.vtxdata[slot_addr]);
            outdata.y = uintBitsToFloat(inputCache.vtxdata[slot_addr + 1]);
            outdata.z = uintBitsToFloat(inputCache.vtxdata[slot_addr + 2]);
            outdata.w = uintBitsToFloat(inputCache.vtxdata[slot_addr + 3]);
            break;
        }
        case 4: {
            // UNORM4
            uint val = inputCache.vtxdata[slot_addr];
            outdata.x = float(bitfieldExtract(val, 0, 8)) / 255.0;
            outdata.y = float(bitfieldExtract(val, 8, 8)) / 255.0;
            outdata.z = float(bitfieldExtract(val, 16, 8)) / 255.0;
            outdata.w = float(bitfieldExtract(val, 24, 8)) / 255.0;
            break;
        }
        case 5: {
            // SNORM4
            int val = int(inputCache.vtxdata[slot_addr]);
            outdata.x = float(bitfieldExtract(val, 0, 8)) / 128.0;
            outdata.y = float(bitfieldExtract(val, 8, 8)) / 128.0;
            outdata.z = float(bitfieldExtract(val, 16, 8)) / 128.0;
            outdata.w = float(bitfieldExtract(val, 24, 8)) / 128.0;
            break;
        }
    }

    return outdata;
}

void main() {
    // registers
    vec4 inputslot[8];
    vec4 odata[4];
    vec4 reg[16];

    // NOTE: output vertex size is 10 words
    // - 4 words for position
    // - 4 words for texcoord 0 UV + texcoord 1 UV
    // - 2 words for col + ocol
    
    uint offset = gl_WorkGroupID.x * (params.stride * params.batch_size);
    uint out_offset = gl_WorkGroupID.x * (10 * params.batch_size);

    for (uint i = 0; i < params.batch_size; i++) {
        uint addr = offset + (i * params.stride);
        uint out_addr = out_offset + (i * 10);

        // load inputs
        for (uint j = 0; j < 8; j++) {
            inputslot[j] = load_vtx_slot(addr, params.slotlayout[j]);
        }

        // process vertex
        for (int j = 0; j < 64; j++) {
            uint instr = params.program[j];

            uint op = instr & 0x3F;
            uint dst = (instr >> 6) & 0xF;
            uint src = (instr >> 10) & 0xF;
            
            uint sx = (instr >> 14) & 3;
            uint sy = (instr >> 16) & 3;
            uint sz = (instr >> 18) & 3;
            uint sw = (instr >> 20) & 3;

            bool mx = ((instr >> 22) & 1) == 1;
            bool my = ((instr >> 23) & 1) == 1;
            bool mz = ((instr >> 24) & 1) == 1;
            bool mw = ((instr >> 25) & 1) == 1;

            switch (op) {
                case 0: {
                    // ld
                    reg[dst] = inputslot[src & 7];
                    break;
                }
                case 1: {
                    // st
                    odata[dst & 3] = reg[src];
                    break;
                }
                case 2: {
                    // ldc
                    reg[dst] = params.cdata[src];
                    break;
                }
                case 3: {
                    // add
                    reg[dst] += reg[src];
                    break;
                }
                case 4: {
                    // sub
                    reg[dst] -= reg[src];
                    break;
                }
                case 5: {
                    // mul
                    reg[dst] *= reg[src];
                    break;
                }
                case 6: {
                    // div
                    reg[dst] /= reg[src];
                    break;
                }
                case 7: {
                    // dot
                    reg[dst] = vec4(dot(reg[dst], reg[src]), 0.0, 0.0, 0.0);
                    break;
                }
                case 8: {
                    // abs
                    reg[dst] = abs(reg[src]);
                    break;
                }
                case 9: {
                    // sign
                    reg[dst] = sign(reg[src]);
                    break;
                }
                case 10: {
                    // sqrt
                    reg[dst] = sqrt(reg[src]);
                    break;
                }
                case 11: {
                    // pow
                    reg[dst] = pow(reg[dst], reg[src]);
                    break;
                }
                case 12: {
                    // exp
                    reg[dst] = exp(reg[src]);
                    break;
                }
                case 13: {
                    // log
                    reg[dst] = log(reg[src]);
                    break;
                }
                case 14: {
                    // min
                    reg[dst] = min(reg[dst], reg[src]);
                    break;
                }
                case 15: {
                    // max
                    reg[dst] = max(reg[dst], reg[src]);
                    break;
                }
                case 16: {
                    // sin
                    reg[dst] = sin(reg[src]);
                    break;
                }
                case 17: {
                    // cos
                    reg[dst] = cos(reg[src]);
                    break;
                }
                case 18: {
                    // tan
                    reg[dst] = tan(reg[src]);
                    break;
                }
                case 19: {
                    // asin
                    reg[dst] = asin(reg[src]);
                    break;
                }
                case 20: {
                    // acos
                    reg[dst] = acos(reg[src]);
                    break;
                }
                case 21: {
                    // atan
                    reg[dst] = atan(reg[src]);
                    break;
                }
                case 22: {
                    // atan2
                    reg[dst] = atan(reg[dst], reg[src]);
                    break;
                }
                case 23: {
                    // shf
                    vec4 v = reg[src];
                    reg[dst] = mix(reg[dst], vec4(v[sx], v[sy], v[sz], v[sw]), bvec4(mx, my, mz, mw));
                    break;
                }
                case 24: {
                    // mulm
                    vec4 c0 = reg[src];
                    vec4 c1 = reg[src + 1];
                    vec4 c2 = reg[src + 2];
                    vec4 c3 = reg[src + 3];
                    reg[dst] = mat4(c0, c1, c2, c3) * reg[dst];
                    break;
                }
            }

            if (op == 0x3F) {
                // end
                break;
            }
        }

        odata[2] = clamp(odata[2], 0.0, 1.0);
        odata[3] = clamp(odata[3], 0.0, 1.0);

        // write to output
        outputCache.vtxdata[out_addr] = floatBitsToUint(odata[0].x);
        outputCache.vtxdata[out_addr + 1] = floatBitsToUint(odata[0].y);
        outputCache.vtxdata[out_addr + 2] = floatBitsToUint(odata[0].z);
        outputCache.vtxdata[out_addr + 3] = floatBitsToUint(odata[0].w);

        outputCache.vtxdata[out_addr + 4] = floatBitsToUint(odata[1].x);
        outputCache.vtxdata[out_addr + 5] = floatBitsToUint(odata[1].y);
        outputCache.vtxdata[out_addr + 6] = floatBitsToUint(odata[1].z);
        outputCache.vtxdata[out_addr + 7] = floatBitsToUint(odata[1].w);

        outputCache.vtxdata[out_addr + 8] =
            bitfieldInsert(0, uint(odata[2].r * 255.0), 0, 8) | 
            bitfieldInsert(0, uint(odata[2].g * 255.0), 8, 8) | 
            bitfieldInsert(0, uint(odata[2].b * 255.0), 16, 8) | 
            bitfieldInsert(0, uint(odata[2].a * 255.0), 24, 8);

        outputCache.vtxdata[out_addr + 9] =
            bitfieldInsert(0, uint(odata[3].r * 255.0), 0, 8) | 
            bitfieldInsert(0, uint(odata[3].g * 255.0), 8, 8) | 
            bitfieldInsert(0, uint(odata[3].b * 255.0), 16, 8) | 
            bitfieldInsert(0, uint(odata[3].a * 255.0), 24, 8);
    }
}