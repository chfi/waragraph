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
layout (set = 1, binding = 2) uniform texture2D u_colors;

layout (set = 1, binding = 3) uniform ColorMap {
  float min_val;
  float max_val;
} u_color_map;

/*
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
*/

layout (set = 1, binding = 3) uniform Transform {
  float a;
  float b;
} u_transform;


void main() {
  uint row_offset = i_slot_id * u_data.row_size;

  float t = i_uv.x;

  t = u_transform.a * t + u_transform.b;

  float c_t = clamp(t, 0.0, 1.0);

  uint data_ix = uint(round(c_t * float(u_data.row_size - 1)));

  float v = u_data.values[row_offset + data_ix];

  v = (v - u_color_map.min_val) / (u_color_map.max_val - u_color_map.min_val);

  vec2 pos = vec2(v, 0.5);
  vec4 color = texture(sampler2D(u_colors, u_sampler), pos);

}

  /*
void main() {

  uint row_offset = i_slot_id * data.row_size;

  float t = i_uv.x;

  t = transform.a * t + transform.b;

  float c_t = clamp(t, 0.0, 1.0);

  uint data_ix = uint(round(c_t * float(data.row_size - 1)));

  float val = data.values[row_offset + data_ix];

  // apply color mapping

  uint c_range_len = color_map.max_color_ix - color_map.min_color_ix;
  float val_range = color_map.max_val - color_map.min_val;

  uint ix = min(uint(round(val)), c_range_len - 1) + 1;

  if (val < color_map.min_val) {
      ix = color_map.extreme_min_color_ix;
  } else if (val > color_map.max_val) {
      ix = color_map.extreme_max_color_ix;
  } else {
      ix = ix + color_map.min_color_ix;
  }

  // infinity is used to signal a bin that doesn't contain any nodes in the path
  if (isinf(val)) {
    f_color = vec4(1.0);
  } else {

    vec4 color = ((t >= 0.0) && (t <= 1.0))
      ? colors.colors[ix]
      : vec4(1.0, 0.0, 0.0, 1.0);

    f_color = color;
  }
}
  */
