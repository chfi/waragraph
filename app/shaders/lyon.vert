#version 450

layout (location = 0) in vec2 a_pos;

layout (set = 0, binding = 0) uniform Transform {
    mat4 m;
} transform;

void main() {
    vec4 pos = vec4(a_pos, 0.0, 1.0);
    gl_Position = transform.m * pos;
}