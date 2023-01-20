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

layout (set = 1, binding = 2) uniform ColorMap {
  uint min_color_ix;
  uint max_color_ix;
  uint extreme_min_color_ix;
  uint extreme_max_color_ix;
  float min_val;
  float max_val;
} color_map;

void main() {
  // float val = data.values[i_node_id];

  f_color = vec4(i_uv, 0.0, 1.0);
}
