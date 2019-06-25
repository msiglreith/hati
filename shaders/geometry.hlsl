
#include "shaders/pack.hlsl"
#include "shaders/resources.hlsl"
#include "shaders/resources_triangle.hlsl"

ConstantBuffer<_DrawData> draw_data : register(b0, space2);

struct GeometryId {
    uint id;
};
ConstantBuffer<GeometryId> geometry_id : register(b1, space2);

struct VsInput {
    float3 pos: Attr0;
};

struct VsOutput {
    float4 pos: SV_Position;
    float3 view_dir : DIRECTION;
    nointerpolation float3 vertex0: VERTEX;
};

VsOutput vs_main(VsInput input) {
    VsOutput output;
    output.pos = mul(proj, mul(view, float4(input.pos.xyz, 1.0)));
    output.view_dir = input.pos - camera_pos;
    output.vertex0 = input.pos;
    return output;
}

uint4 ps_main(
    VsOutput input,
    uint prim_id: SV_PrimitiveID
) : SV_TARGET0 {
    uint index0 = 3 * prim_id + draw_data.base_index;
    uint e1 = index_buffer.Load(index0 + 1);
    uint e2 = index_buffer.Load(index0 + 2);

    float3 vertex0 = input.vertex0;
    float3 vertex1 = vertex_buffer_position.Load(draw_data.base_vertex + e1);
    float3 vertex2 = vertex_buffer_position.Load(draw_data.base_vertex + e2);

    float3 barycentric = raycast_triangle_barycentric(
        camera_pos.xyz,
        input.view_dir,
        vertex0, vertex1, vertex2
    );

    return uint4(
        prim_id,
        geometry_id.id,
        pack_barycentric_f16(barycentric.xy)
    );
}
