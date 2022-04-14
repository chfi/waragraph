#version 450

// layout (location = 0) in vec4 v_color;

layout (location = 0) out vec4 f_color;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  f_color = vec4(1.0, 0.0, 0.0, 1.0);
  // f_color = v_color;
}
