// Shared shader resource interfaces to avoid collisions
//
// All of these interfaces use space0.
// Other shader resource interfaces should use another space!

cbuffer ViewData : register(b0, space0) {
    float4x4 view;
    float4x4 proj;
    float4 camera_pos;
};
