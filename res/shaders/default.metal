#include <metal_stdlib>

using namespace metal;

struct Vertex {
    float3 loc [[attribute(0)]];
};

struct Constants {
    float4x4 proj;
    float4x4 view;
};

struct InstanceData {
    float4x4 model;
};

struct RasterizerData {
    float4 position [[position]];
};

vertex RasterizerData
vertex_shader(
    Vertex vert [[stage_in]],
    constant ushort& iid [[buffer(4)]],
    constant Constants& constants [[buffer(1)]],
    constant InstanceData& instance [[buffer(2)]],
    constant float3* locations [[buffer(3)]]
) {
    RasterizerData out;
    out.position = constants.proj
        * constants.view
        * instance.model
        * float4(vert.loc + locations[iid], 1.0);
    return out;
}

fragment float4
fragment_shader(RasterizerData in [[stage_in]]) {
    return float4(1.0, 1.0, 1.0, 1.0);
}
