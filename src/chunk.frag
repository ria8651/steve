#version 450

layout(location = 0) out vec4 o_Target;
layout(location = 0) in float v_ao;
layout(location = 1) in vec3 v_normal;

void main() {
    float light = clamp(clamp(dot(v_normal, vec3(1.0, 0.7, 0.3)), 0.0, 1.0) + 0.3, 0.0, 1.0) * v_ao;
    vec3 colour = vec3(0.098, 0.6549, 0.1451) * light;
    o_Target = vec4(colour, 1.0);
}