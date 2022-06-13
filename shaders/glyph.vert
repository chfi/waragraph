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

  float width = glyph_size.x / inputs.window_dims.x;
  float height = glyph_size.y / inputs.window_dims.y;
  vec2 origin = (2.0 * glyph_position / inputs.window_dims) - vec2(1.0);

  gl_Position = vec4(origin, 0.0, 1.0);
  o_uv = vec2(0, 0);

  /*
  if (i == 0) {
      gl_Position = vec4(origin, 0.0, 1.0);
      o_uv = vec2(0, 0);
  } else if (i == 1 || i == 4) {
      float x = 8.0 * text_offset.y;
      gl_Position = vec4(origin + vec2(width, 0), 0.0, 1.0);
      o_uv = vec2(x, 0);
  } else if (i == 2 || i == 3) {
      gl_Position = vec4(origin + vec2(0, height), 0.0, 1.0);
      o_uv = vec2(0, 8.0);
  } else {
      float x = 8.0 * text_offset.y;
      gl_Position = vec4(origin + vec2(width, height), 0.0, 1.0);
      o_uv = vec2(x, 8.0);
  }
  */

  o_color = i_color;
}
