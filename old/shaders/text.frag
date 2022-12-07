#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) flat in uvec2 i_text_offset;
layout (location = 2) in vec4 i_color;

layout (location = 0) out vec4 f_color;

layout (set = 0, binding = 0) uniform texture2D font_img;
layout (set = 0, binding = 1) uniform sampler u_sampler;

layout (set = 1, binding = 0) buffer TextData {
  // uint len;
  uint packed_chars[];
} text;

layout (push_constant) uniform Inputs {
  vec2 window_dims;
} inputs;

vec2 offset_for_char(in uint packed_char, in uint offset) {
  uint char_ix = (packed_char >> (offset * 8)) & 127;
  return vec2(char_ix * 8, 0);
}

void main() {
  uint char_ix = i_text_offset.x + uint(i_uv.x) / 8;

  uint packed_ix = char_ix / 4;
  uint packed_offset = char_ix % 4;

  vec2 tex_origin = offset_for_char(text.packed_chars[packed_ix],
                                    packed_offset);

  vec2 offset = vec2(uint(i_uv.x) % 8 + fract(i_uv.x), i_uv.y);

  float r = offset.x / 8.0;
  float b = offset.y / 8.0;

  float g = 0.0;

  vec2 char_px = (tex_origin + offset) / vec2(1024.0, 8.0);

  float value = texture(sampler2D(font_img, u_sampler), char_px).r;

  f_color = vec4(i_color.rgb, 1.0 - value);
}
