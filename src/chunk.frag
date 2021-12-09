#version 450

layout(location = 0) out vec4 o_Target;
layout(location = 0) in float v_ao;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;

layout(set = 2, binding = 0) uniform texture2D ChunkMaterial_texture_atlas;
layout(set = 2, binding = 1) uniform sampler ChunkMaterial_texture_atlas_sampler;

void main() {
    vec4 texture_colour = texture(sampler2D(ChunkMaterial_texture_atlas, ChunkMaterial_texture_atlas_sampler), v_uv);
    float light = clamp(clamp(dot(v_normal, vec3(1.0, 0.7, 0.3)), 0.0, 1.0) + 0.3, 0.0, 1.0) * v_ao;
    vec3 colour = texture_colour.xyz * light;
    o_Target = vec4(colour, 1.0);
}