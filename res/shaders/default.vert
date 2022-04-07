#version 450

layout(push_constant) uniform Constants {
    mat4 proj;
    mat4 view;
};

layout(binding = 0) uniform InstanceData {
    mat4 model;
};

layout(location = 0) in vec3 position;

void main()
{
    gl_Position = proj * view * model * vec4(position, 1.0);
}
