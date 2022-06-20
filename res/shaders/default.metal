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

struct GuestData {
    float4 loc;
    float4 goal;
};

struct RasterizerData {
    float4 position [[position]];
};

vertex RasterizerData
vertexShader(
    Vertex vert [[stage_in]],
    constant ushort& iid [[buffer(4)]],
    constant Constants& constants [[buffer(1)]],
    constant InstanceData& instance [[buffer(2)]],
    constant GuestData* guest [[buffer(3)]]
) {
    RasterizerData out;
    out.position = constants.proj
        * constants.view
        * instance.model
        * vector_float4(vert.loc.x + guest[iid].loc.x, vert.loc.y + guest[iid].loc.y, vert.loc.z + guest[iid].loc.z, 1.0);
    return out;
}

fragment float4
fragmentShader(RasterizerData in [[stage_in]]) {
    return vector_float4(1.0, 1.0, 1.0, 1.0);
}
