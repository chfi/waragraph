#version 450

layout (location = 0) in uint a_node_id;
layout (location = 1) in vec2 a_p0;
layout (location = 2) in vec2 a_p1;

layout (location = 0) out uint o_node_id;
layout (location = 1) out vec2 o_uv;

layout (set = 0, binding = 0) uniform Transform {
    mat4 m;
} transform;

layout (set = 0, binding = 1) uniform Config {
    vec2 viewport_dimensions;
    float node_width;
} config;

void main() {

     vec4 start = transform.m * vec4(a_p0, 0.0, 1.0);
     vec4 end = transform.m * vec4(a_p1, 0.0, 1.0);

     vec4 diff = end - start;
     vec4 dir = normalize(diff);
     
     
}


