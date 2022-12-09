#version 450

layout (location = 0) in vec2 i_uv;
// layout (location = 1) in flat uvec2 i_offset_len;

layout (location = 0) out vec4 f_color;

/*
layout (set = 0, binding = 0) readonly buffer DataBuf {
  uint val[];
} data;

layout (set = 1, binding = 0) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;
*/

layout (set = 1, binding = 0) uniform FragCfg {
    vec4 color;
} cfg;

void main() {
  f_color = cfg.color;
  /*
  uint offset = i_offset_len.x / 4;
  uint len = i_offset_len.y / 4;
  uint ix = offset + uint(floor(i_uv.x * len));
  uint val = data.val[ix];
  vec4 color = colors.colors[val % colors.len];
  f_color = color;
  */
}
