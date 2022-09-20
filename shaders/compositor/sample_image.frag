#version 450

layout (location = 0) in vec2 i_uv;
// layout (location = 1) in vec2 i_size;
// layout (location = 2) in vec4 i_color;

layout (location = 0) out vec4 f_color;

layout (set = 0, binding = 0) uniform sampler u_sampler;

layout (set = 1, binding = 0) uniform texture2D u_src;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {

  vec2 pos = gl_FragCoord.xy / inputs.window_dims;
  
  // f_color = vec4(1.0, pos.x, pos.y, 1.0);
  // f_color = texture(sampler2D(u_src, u_sampler), i_uv);

  f_color = texture(sampler2D(u_src, u_sampler), pos * vec2(1024));
}
