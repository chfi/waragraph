#version 450

layout (location = 0) in vec2 i_position;
layout (location = 1) in vec2 i_size;
layout (location = 2) in uint i_buffer_len;

layout (location = 0) out vec2 o_uv;
layout (location = 1) out uint o_buffer_len;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  uint i = gl_VertexIndex % 6;

  vec2 origin = (2.0 * i_position / inputs.window_dims) - vec2(1.0);
  vec2 size = (2.0 * i_size) / inputs.window_dims;

  if (i == 0) {
      gl_Position = vec4(origin, 0.0, 1.0);
      o_uv = vec2(0.0, 0.0);
  } else if (i == 1 || i == 4) {
      gl_Position = vec4(origin + vec2(size.x, 0), 0.0, 1.0);
      o_uv = vec2(1.0, 0.0);
  } else if (i == 2 || i == 3) {
      gl_Position = vec4(origin + vec2(0, size.y), 0.0, 1.0);
      o_uv = vec2(0.0, 1.0);
  } else {
      gl_Position = vec4(origin + size, 0.0, 1.0);
      o_uv = vec2(1.0, 1.0);
  }

  o_buffer_len = i_buffer_len;
}
