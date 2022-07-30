#version 450

layout (location = 0) in vec2 p0;
layout (location = 1) in float p0_w;
layout (location = 2) in vec2 p1;
layout (location = 3) in float p1_w;
layout (location = 4) in vec4 color0;
layout (location = 5) in vec4 color1;

layout (location = 0) out vec4 o_color;

// layout (set = 0, binding = 0) uniform VP {
//   mat4 view_proj;
// }

// layout (set = 0, binding = 0) uniform UBO {
//   mat4 model_t;
// } ubo;

layout (set = 0, binding = 0) uniform UBO {
  mat4 proj;
  vec2 offset;
  float scale;
} ubo;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  uint i = gl_VertexIndex % 6;

  vec2 u = p1.xy - p0.xy;
  float len = length(u);
  vec2 n = u / len;

  vec2 p0_ = ubo.proj * (p0 - offset);
  vec2 p1_ = ubo.proj * (p1 - offset);

  vec2 a = p0_ + vec2(-n.y, n.x) * p0_w;
  vec2 b = p1_ + vec2(-n.y, n.x) * p1_w;
  vec2 c = p1_ - vec2(-n.y, n.x) * p1_w;
  vec2 d = p0_ - vec2(-n.y, n.x) * p0_w;

  vec2 pos = vec2(0.0);

  if (i == 0 || i == 5) {
    pos =
      (2.0 * a / inputs.window_dims) - vec2(1.0);
    o_color = color0;
  } else if (i == 1) {
    pos =
      (2.0 * b / inputs.window_dims) - vec2(1.0);
    o_color = color1;
  } else if (i == 2 || i == 3) {
    pos =
      (2.0 * c / inputs.window_dims) - vec2(1.0);
    o_color = color1;
  } else {
    pos =
      (2.0 * d / inputs.window_dims) - vec2(1.0);
    o_color = color0;
  }

  gl_Position = vec4(pos, 0.0, 1.0);
}
