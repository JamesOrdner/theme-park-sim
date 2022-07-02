using namespace metal;

kernel void
update_guest_locations(
    uint index [[thread_position_in_grid]],
    constant float& delta_time [[buffer(0)]],
    constant float4* locations [[buffer(1)]],
    constant float4* velocities [[buffer(2)]],
    device float4* out_locations [[buffer(3)]]
) {
    out_locations[index] = locations[index] + velocities[index] * delta_time;
    out_locations[index].w = 1.0;
}
