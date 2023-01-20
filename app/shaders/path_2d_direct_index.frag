#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_node_id;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer NodeData {
  float values[];
} data;

layout (set = 1, binding = 1) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;

void main() {
  float val = data.values[i_node_id];

  uint ix = uint(round(val)) % colors.len;

  f_color = colors.colors[ix];
}
