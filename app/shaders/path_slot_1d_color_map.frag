#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_slot_id;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer DataBuf {
  uint total_size;
  uint row_size;
  float values[];
} u_data;

layout (set = 1, binding = 1) uniform sampler u_sampler;
layout (set = 1, binding = 2) uniform texture1D u_colors;

layout (set = 1, binding = 3) uniform ColorMap {
  float min_val;
  float max_val;
  float min_color;
  float max_color;
} u_color_map;

layout (set = 1, binding = 4) readonly buffer Transform {
  vec2 ab[];
} u_transforms;

void main() {
  uint row_offset = i_slot_id * u_data.row_size;

  float t = i_uv.x;

  vec2 ab = u_transforms.ab[i_slot_id];
  t = ab.x * t + ab.y;

  float c_t = clamp(t, 0.0, 1.0);

  uint data_ix = uint(round(c_t * float(u_data.row_size - 1)));

  float v = u_data.values[row_offset + data_ix];

  float v_n = (v - u_color_map.min_val) / (u_color_map.max_val - u_color_map.min_val);

  float c_n = mix(u_color_map.min_color, u_color_map.max_color, v_n);

  // v = (v - u_color_map.min_val) / (u_color_map.max_val - u_color_map.min_val);

  vec4 color = texture(sampler1D(u_colors, u_sampler), c_n);

  f_color = color;

}
