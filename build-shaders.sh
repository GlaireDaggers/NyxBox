mkdir -p ./content/shaders/
./tools/linux/glslc -fshader-stage=compute ./shaders-src/vu.glsl -o ./content/shaders/vu.spv