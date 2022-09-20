#version 450

layout (location = 0) in vec2 p0;
layout (location = 1) in vec2 p1;
layout (location = 2) in uint node_len; // in bp


layout (location = 0) out uint o_node_index;
// bp-unit
layout (location = 1) out float o_bp;
// vertical UV along the node
layout (location = 2) out float o_v;

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
  float node_width;
} inputs;

void main() {
  uint i = gl_VertexIndex % 6;

  vec2 u = p1.xy - p0.xy;
  float len = length(u);
  vec2 n = u / len;

  float scale = 1.0 / ubo.scale;
  // float scale = ubo.scale;

  vec4 q0 = vec4(p0.xy - ubo.offset, 0.0, 1.0);
  vec4 q1 = vec4(p1.xy - ubo.offset, 0.0, 1.0);

  mat4 scaling = mat4(scale, 0.0, 0.0, 0.0,
                      0.0, scale, 0.0, 0.0,
                      0.0,   0.0, 1.0, 1.0,
                      0.0,   0.0, 0.0, 1.0);

  vec4 p0_ = ubo.proj * scaling * q0;
  vec4 p1_ = ubo.proj * scaling * q1;

  float aspect = inputs.window_dims.y / inputs.window_dims.x;

  vec2 n_a = vec2(-n.y, n.x / aspect);

  float n_w = inputs.node_width;

  vec2 p0_na = n_a * n_w / inputs.window_dims.x;
  vec2 p1_na = n_a * n_w / inputs.window_dims.x;

  vec2 a = p0_.xy + p0_na;
  vec2 b = p1_.xy + p1_na;
  vec2 c = p1_.xy - p1_na;
  vec2 d = p0_.xy - p0_na;

  vec2 pos = vec2(0.0);

  if (i == 0) {
    // top left
    pos = a;
    o_bp = 0.0;
    o_v = 0.0;
  } else if (i == 1 || i == 4) {
    // top right
    pos = b;
    o_bp = float(node_len);
    o_v = 0.0;
  } else if (i == 2 || i == 5) {
    // bottom left
    pos = d;
    o_bp = 0.0;
    o_v = 1.0;
  // } else if (i == 4) {
  } else {
    // bottom right
    pos = c;
    o_bp = float(node_len);
    o_v = 1.0;
  }

  // or should this be instance index
  // o_node_index = gl_VertexIndex / 6;

  o_node_index = gl_InstanceIndex;

  // gl_Position = vec4(pos, 0.0, 1.0);

  // gl_Position = vec4(pos + vec2(1.0), 0.0, 1.0);
  gl_Position = vec4(pos + vec2(0.5), 0.0, 1.0);
}
