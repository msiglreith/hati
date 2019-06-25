
Texture2D<float4> g_input_hdr : register(t0, space0);
SamplerState g_sampler : register(s0, space0);

struct VsOutput {
    float4 pos: SV_Position;
    float2 uv: TEXCOORD0;
};

VsOutput vs_main(uint id: SV_VertexID) {
    float2 uv = float2((id << 1) & 2, id & 2);
    VsOutput output = {
        float4(float2(-1.0, 1.0) + uv * float2(2.0, -2.0), 0.0, 1.0),
        uv
    };
    return output;
}

float4 ps_displaymap(VsOutput input) : SV_Target0 {
    const float exposure = 8.0f; // TODO:
    float3 color = g_input_hdr.SampleLevel(g_sampler, input.uv, 0).xyz;
    color *= exposure;

    return float4(color / (1.0 + color), 1.0);
}
