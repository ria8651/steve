#version 450

layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec3 Vertex_Normal;
layout(location = 2) in vec2 Vertex_UV;
layout(location = 3) in float Vertex_AO;
layout(location = 0) out float v_ao;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;


layout(set = 0, binding = 0) uniform CameraViewProj {
    mat4 ViewProj;
};

layout(set = 1, binding = 0) uniform Transform {
    mat4 Model;
};

void main() {
    gl_Position = ViewProj * Model * vec4(Vertex_Position, 1.0);
    v_ao = Vertex_AO;
    v_normal = Vertex_Normal;
    v_uv = Vertex_UV;
}