#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in uvec2 text_offset;
layout (location = 2) in vec4 color;

// layout (location = 1) in vec4 color;
// layout (location = 1) in float i_color;

layout (location = 0) out uvec2 o_uv;
layout (location = 1) out uvec2 o_text_offset;
layout (location = 2) out vec4 o_color;
// layout (location = 1) out vec2 o_uv;

/*
layout (set = 0, binding = 0) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;
*/

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {

  uint i = gl_VertexIndex % 6;

  vec2 origin = (2.0 * position / inputs.window_dims) - vec2(1.0);

  if (i == 0) {
      gl_Position = vec4(origin, 0.0, 1.0);
      o_uv = uvec2(0, 0);
  } else if (i == 1 || i == 3) {
      uint x = 8 * text_offset.y;
      gl_Position = vec4(origin + vec2(x, 0), 0.0, 1.0);
      o_uv = uvec2(x, 0);
  } else if (i == 2 || i == 5) {
      gl_Position = vec4(origin + vec2(0, 8), 0.0, 1.0);
      o_uv = uvec2(0, 8);
  } else if (i == 4) {
      uint x = 8 * text_offset.y;
      gl_Position = vec4(origin + vec2(x, 8), 0.0, 1.0);
      o_uv = uvec2(x, 8);
  }
  /*
  if i == 0 {
  } else if i == 1 {
  } else if i == 2 {
  } else {
  }
  */

      // gl_Position = vec4(pos, 0.0, 1.0);


  // uint color_ix = uint(i_color);
  // o_color = colors.colors[4];
  o_text_offset = text_offset;
  o_color = color;
}
