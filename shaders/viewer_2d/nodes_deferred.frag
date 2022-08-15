#version 450

layout (location = 0) in flat uint i_node_index;
// bp-unit
layout (location = 1) in float i_bp;
// vertical UV along the node
layout (location = 2) in float i_v;

// layout (location = 0) in vec4 i_color;

layout (location = 0) out uint o_node_index;
layout (location = 1) out vec2 o_bp_v;

// layout (set = 0, binding = 0) readonly buffer Colors {
//   vec4 color[];
// } node_colors;

// layout (push_constant) uniform Input {
//   vec2 window_dims;
// } inputs;

void main() {
  o_node_index = i_node_index;
  o_bp_v = vec2(i_bp, i_v);
}
