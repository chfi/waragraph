#version 450

layout (location = 0) in vec2 p0;
layout (location = 1) in vec2 p1;
layout (location = 2) in float width;
layout (location = 3) in vec4 color;

layout (location = 0) out vec4 o_color;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  uint i = gl_VertexIndex % 6;

  vec2 u = p1 - p0;
  float len = length(u);
  vec2 n = u / len;

  vec2 a = p0 + vec2(-n.y, n.x) * width;
  vec2 b = p1 + vec2(-n.y, n.x) * width;
  vec2 c = p1 - vec2(-n.y, n.x) * width;
  vec2 d = p0 - vec2(-n.y, n.x) * width;

  vec2 pos = vec2(0.0);

  if (i == 0 || i == 5) {
    pos =
      (2.0 * a / inputs.window_dims) - vec2(1.0);
  } else if (i == 1) {
    pos =
      (2.0 * b / inputs.window_dims) - vec2(1.0);
  } else if (i == 2 || i == 3) {
    pos =
      (2.0 * c / inputs.window_dims) - vec2(1.0);
  } else {
    pos =
      (2.0 * d / inputs.window_dims) - vec2(1.0);
  }

  gl_Position = vec4(pos, 0.0, 1.0);

  o_color = color;
}
