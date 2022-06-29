#version 450

layout (location = 0) in vec3 p0;
layout (location = 1) in vec3 p1;
layout (location = 2) in vec4 color0;
layout (location = 3) in vec4 color1;

layout (location = 0) out vec4 o_color;

// layout (set = 0, binding = 0) uniform VP {
//   mat4 view_proj;
// }

// layout (set = 0, binding = 0) uniform UBO {
//   mat4 model_t;
// } ubo;

layout (set = 0, binding = 0) uniform UBO {
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

  vec2 a = p0.xy + vec2(-n.y, n.x) * p0.z;
  vec2 b = p1.xy + vec2(-n.y, n.x) * p1.z;
  vec2 c = p1.xy - vec2(-n.y, n.x) * p1.z;
  vec2 d = p0.xy - vec2(-n.y, n.x) * p0.z;

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
