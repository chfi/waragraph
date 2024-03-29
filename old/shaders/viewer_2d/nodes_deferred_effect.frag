#version 450

layout (location = 0) in vec2 i_uv;

layout (location = 0) out vec4 o_color;

layout (set = 0, binding = 0) uniform sampler u_sampler;
layout (set = 0, binding = 1) uniform utexture2D u_index_img;
// layout (set = 0, binding = 1, r32ui) uniform uimage2D u_index_img;
layout (set = 0, binding = 2) uniform texture2D u_uv_img;

// layout (set = 0, binding = 0) readonly buffer Colors {
//   vec4 color[];
// } node_colors;

layout (push_constant) uniform Input {
  vec2 out_dims;
} inputs;

void main() {
  ivec2 pixel = ivec2(inputs.out_dims * i_uv);

  uint node_id = texelFetch(usampler2D(u_index_img, u_sampler), pixel, 0).r;

  vec4 bp_v = texture(sampler2D(u_uv_img, u_sampler), i_uv);

  int bp = int(bp_v.x);

  float bpx = float(bp % 250) / 250.0;

  o_color = (node_id == 0xffffffff) ? vec4(0.0) : vec4(bpx, 0.0, 0.0, 1.0);
  // o_color = (node_id == 0xffffffff) ? vec4(0.0) : vec4(bp % 250, 0.0, 0.0, 1.0);
}
