#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in vec2 i_size;
layout (location = 2) in vec4 i_color;

layout (location = 0) out vec4 f_color;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  f_color = i_color;
}
