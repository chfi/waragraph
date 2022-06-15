#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in vec4 i_color;

layout (location = 0) out vec4 f_color;

layout (set = 0, binding = 0) uniform sampler u_sampler;
layout (set = 0, binding = 1) uniform texture2D u_image_in;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  float alpha = texture(sampler2D(u_image_in, u_sampler), i_uv).r;

  if (alpha > 0.0) {
    vec3 base = i_color.rgb * alpha;
    f_color = vec4(base, alpha);
  } else {
    f_color = vec4(0.0);
  }
}
