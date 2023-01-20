#version 450

layout (location = 0) in vec2 a_p0;
layout (location = 1) in vec2 a_p1;
layout (location = 2) in uint a_node_id;

layout (location = 0) out vec2 o_uv;
layout (location = 1) out uint o_node_id;

layout (set = 0, binding = 0) uniform Transform {
    mat4 m;
} transform;

layout (set = 0, binding = 1) uniform Config {
    float node_width;
} config;


void main() {
  o_node_id = a_node_id;

  vec4 start = transform.m * vec4(a_p0, 0.0, 1.0);
  vec4 end = transform.m * vec4(a_p1, 0.0, 1.0);

  vec4 diff = vec4(a_p1, 0.0, 1.0) - vec4(a_p0, 0.0, 1.0);
  vec4 dir = normalize(diff);

  mat4x4 rot = mat4x4(0, -1, 0, 0,
                      1, 0, 0, 0,
                      0, 0, 1, 0,
                      0, 0, 0, 1);


  // vec4 magic = transform.m * vec4(0.0, config.node_width, 0.0, 0.0);
  vec4 perp = rot * dir * config.node_width;

     
  float v = config.node_width;

  vec4 sl = start + 0.5 * perp * v;
  vec4 sr = start - 0.5 * perp * v;

  vec4 el = end + 0.5 * perp * v;
  vec4 er = end - 0.5 * perp * v;

     
  uint i = gl_VertexIndex % 6;
  if (i == 0) {
    // start left
    gl_Position = sl;
    o_uv = vec2(0.0);
  } else if (i == 1 || i == 4) {
    // end left
    gl_Position = el;
    o_uv = vec2(0.0, 1.0);
  } else if (i == 2 || i == 5) {
    // start right
    gl_Position = sr;
    o_uv = vec2(0.0, 1.0);
  } else {
    // end right
    gl_Position = er;
    o_uv = vec2(1.0);
  }
     
     
}


