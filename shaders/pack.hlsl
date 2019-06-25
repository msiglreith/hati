
#define F16_MAX 65504.0

uint2 pack_barycentric_f16(float2 uv) {
    return f32tof16(uv * F16_MAX);
}

float2 unpack_barycentric_f16(uint2 uv) {
    return f16tof32(uv) / F16_MAX;
}