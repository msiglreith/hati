// Triangle reconstruction resources
//
// Using space1

struct _DrawData {
    uint base_index;
    uint base_vertex;
};

StructuredBuffer<uint> index_buffer: register(t0, space1);
StructuredBuffer<float3> vertex_buffer_position: register(t1, space1);

// Möller–Trumbore intersection
float3 raycast_triangle_barycentric(float3 origin, float3 dir, float3 v0, float3 v1, float3 v2) {
    float3 e0 = v1 - v0;
    float3 e1 = v2 - v0;
    float3 h = cross(dir, e1);
    float f = 1.0 / dot(e0, h);
    float3 s = origin - v0;
    float3 q = cross(s, e0);

    float bary_v = f * dot(s, h);
    float bary_w = f * dot(dir, q);

    return float3(1.0 - bary_v - bary_w, bary_v, bary_w);
}
