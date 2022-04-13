#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec4 color;

layout (location = 0) out vec4 o_color;
// layout (location = 1) out vec2 o_uv;

void main() {
  gl_Position = vec4(position.xy, 0.0, 1.0);
  o_color = color;

  // vec2 uv = (


  // if (gl_VertexIndex % 4

}
