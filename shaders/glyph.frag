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


  vec4 nbors = textureGather(sampler2D(u_image_in, u_sampler), i_uv, 0);

  float nbor_alpha = (nbors.r + nbors.g + nbors.b + nbors.a) / 4.0;

  float a = 0.8 * alpha + 0.2 * nbor_alpha;

  // if (a > 0.0) {
  //   vec3 base = i_color.rgb * a;
  //   f_color = vec4(base, 1.0);
  // } else {
  //   f_color = vec4(0.0);
  // }

  if (alpha > 0.0) {
    // vec3 base = i_color.rgb * alpha;
    vec3 base = i_color.rgb * a;
    // vec3 base = i_color.rgb * (1.0 - alpha);
    // f_color = vec4(base, 1.0 - alpha);
    // f_color = vec4(base, alpha);
    f_color = vec4(base, a);
  } else {
    f_color = vec4(0.0);
  }
}
