using namespace metal;

struct GuestData {
    float4 loc;
    float4 goal; // coord w is speed
};

kernel void
update_guest_locations(
    uint index [[thread_position_in_grid]],
    constant float& delta_time [[buffer(0)]],
    device GuestData* data [[buffer(1)]]
) {
    if (data[index].goal.w != 0.0) {
        float3 delta_loc = normalize(vector_float3(data[index].goal - data[index].loc))
            * data[index].goal.w
            * delta_time;

        data[index].loc.x += delta_loc.x;
        data[index].loc.y += delta_loc.y;
        data[index].loc.z += delta_loc.z;
    }
}
