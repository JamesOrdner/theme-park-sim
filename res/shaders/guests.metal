using namespace metal;

kernel void
update_guest_velocities(
    uint index [[thread_position_in_grid]],
    constant float4* frame_updates [[buffer(0)]],
    device float3* velocities [[buffer(1)]]
) {
    const float3 velocity = frame_updates[index].xyz;
    const uint32_t instance_index = as_type<uint32_t>(frame_updates[index].w);

    velocities[instance_index] = velocity;
}

kernel void
update_guest_locations(
    uint index [[thread_position_in_grid]],
    constant float& delta_time [[buffer(0)]],
    constant float3* velocities [[buffer(1)]],
    constant float3* locations [[buffer(2)]],
    device float3* out_locations [[buffer(3)]]
) {
    out_locations[index] = locations[index] + velocities[index] * delta_time;
}
