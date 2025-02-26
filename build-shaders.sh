mkdir -p ./content/shaders/
./tools/linux/glslc -fshader-stage=compute ./shaders-src/vu.glsl -o ./content/shaders/vu.spv
./tools/linux/glslc -fshader-stage=compute ./shaders-src/draw_tri_list.glsl -o ./content/shaders/draw_tri_list.spv