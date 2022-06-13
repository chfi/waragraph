#version 450

layout (location = 0) in vec2 glyph_position;
layout (location = 1) in vec2 glyph_size;

layout (location = 2) in vec2 uv_pos;
layout (location = 3) in vec2 uv_size;

layout (location = 4) in vec4 i_color;


layout (location = 0) out vec2 o_uv;
layout (location = 1) out vec4 o_color;


layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  uint i = gl_VertexIndex % 6;

  vec2 dims = 2.0 * glyph_size / inputs.window_dims;
  // vec2 dims = glyph_size / inputs.window_dims;

  float width = dims.x;
  float height = dims.y;

  vec2 origin = (2.0 * glyph_position / inputs.window_dims) - vec2(1.0);

  gl_Position = vec4(origin, 0.0, 1.0);
  o_uv = vec2(0, 0);


  if (i == 0) {
    // top left
      gl_Position = vec4(origin, 0.0, 1.0);
      o_uv = uv_pos;
  } else if (i == 1 || i == 4) {
    // top right
      gl_Position = vec4(origin + vec2(width, 0), 0.0, 1.0);
      o_uv = uv_pos + vec2(uv_size.x, 0);
  } else if (i == 2 || i == 3) {
    // bottom right
      gl_Position = vec4(origin + vec2(0, height), 0.0, 1.0);
      o_uv = uv_pos + vec2(0, uv_size.y);
  } else {
    // bottom left
      gl_Position = vec4(origin + vec2(width, height), 0.0, 1.0);
      o_uv = uv_pos + uv_size;
  }

  o_color = i_color;
}
