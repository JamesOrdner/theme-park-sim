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
    constant float4* guest [[buffer(3)]]
) {
    RasterizerData out;
    out.position = constants.proj
        * constants.view
        * instance.model
        * vector_float4(vert.loc.x + guest[iid].x, vert.loc.y + guest[iid].y, vert.loc.z + guest[iid].z, 1.0);
    return out;
}

fragment float4
fragment_shader(RasterizerData in [[stage_in]]) {
    return vector_float4(1.0, 1.0, 1.0, 1.0);
}
