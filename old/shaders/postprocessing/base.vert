#version 450 

layout (location = 0) out vec2 o_uv;

void main() 
{
    o_uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(o_uv * 2.0f + -1.0f, 0.0f, 1.0f);
}
