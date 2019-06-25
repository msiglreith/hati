
#include "shaders/pack.hlsl"
#include "shaders/resources_triangle.hlsl"

// Draw information ( + triangle resources) ----------------------- space 1
StructuredBuffer<_DrawData> g_draw_data : register(t2, space1);

// Texture data --------------------------------------------------- space 2
Texture2D<float4> textures[] : register(t0, space2);
SamplerState sampler_texture : register(s0, space2);

// Input/Ouput render targets ------------------------------------- space 3
RWTexture2D<float4> lighting_buffer : register(u0, space3);
Texture2D<uint4> geometry_buffer : register(t1, space3);

// Light information ---------------------------------------------- space 4
struct LightData {
    uint num_point_lights;
};
ConstantBuffer<LightData> light_data : register(b0, space4);

struct PointLight {
    float3 position;
    float intensity; // TODO: unit luminous intensity
};
StructuredBuffer<PointLight> point_lights : register(t0, space4);


[numthreads(16, 16, 1)]
void cs_lighting(
    uint3 thread_id: SV_DispatchThreadID,
    uint3 tile_thread_id: SV_GroupThreadID
) {
    uint4 geometry = geometry_buffer.Load(uint3(thread_id.xy, 0));
    uint prim_id = geometry.x;
    uint geometry_id = geometry.y;

    // Reconstruct triangle -----------------------------------------
    _DrawData draw_data = g_draw_data[geometry_id];

    uint index0 = 3 * prim_id + draw_data.base_index;
    uint e0 = index_buffer.Load(index0);
    uint e1 = index_buffer.Load(index0 + 1);
    uint e2 = index_buffer.Load(index0 + 2);

    float3 vertex0 = vertex_buffer_position.Load(draw_data.base_vertex + e0);
    float3 vertex1 = vertex_buffer_position.Load(draw_data.base_vertex + e1);
    float3 vertex2 = vertex_buffer_position.Load(draw_data.base_vertex + e2);

    // Reconstruct barycentrics
    float2 barycentrics = unpack_barycentric_f16(geometry.zw);
    float bary_u = barycentrics.x;
    float bary_v = barycentrics.y;
    float bary_w = 1.0 - bary_u - bary_v;

    float3 world_position = vertex0 * bary_u + vertex1 * bary_v + vertex2 * bary_w;
    float3 lighting = float3(0.0, 0.0, 0.0);

    // Accumulate lighting -----------------------------------------
    // Point lights
    for (uint i = 0; i < light_data.num_point_lights; i++) {
        PointLight point_light = point_lights[i];
        float3 v_light = point_light.position - world_position;
        float distSq = dot(v_light, v_light);
        float light = point_light.intensity / distSq;

        lighting += float3(light, light, light);
    }

    lighting_buffer[thread_id.xy] = float4(lighting, 0);
}