#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_slot_id;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer DataBuf {
  uint row_size;
  uint total_size;
  uint values[];
} data;

layout (set = 1, binding = 1) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;

void main() {
  uint row_offset = i_slot_id * data.row_size;
  uint data_ix = uint(i_uv.x * float(data.row_size - 1));
  uint val = data.values[data_ix];
  uint ix = min(val, colors.len - 1);
  vec4 color = colors.colors[ix];
  f_color = color;
}
