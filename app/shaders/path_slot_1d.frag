#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_slot_id;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer DataBuf {
  uint total_size;
  uint row_size;
  float values[];
} data;

layout (set = 1, binding = 1) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;

layout (set = 1, binding = 2) uniform Transform {
  float a;
  float b;
} transform;

void main() {
  uint row_offset = i_slot_id * data.row_size;

  float t = i_uv.x;

  t = transform.a * t + transform.b;

  float c_t = clamp(t, 0.0, 1.0);

  uint data_ix = uint(round(c_t * float(data.row_size - 1)));
  // uint data_ix = uint(i_uv.x * float(data.row_size - 1));

  float val = data.values[row_offset + data_ix];


  if (isinf(val)) {
    // f_color = vec4(1.0);
    f_color = vec4(0.5, 0.0, 0.7, 1.0);
  } else {
    uint ix = min(uint(round(val)), colors.len - 2) + 1;

    vec4 color = ((t >= 0.0) && (t <= 1.0))
      ? colors.colors[ix]
      : vec4(1.0, 0.0, 0.0, 1.0);
    // vec4 color = colors.colors[ix];
    f_color = color;
  }
  
}
