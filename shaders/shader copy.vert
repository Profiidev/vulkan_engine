#version 450

layout (location=0) in vec3 position;
layout (location=1) in vec3 normal;
layout (location=2) in mat4 model_matrix;
layout (location=6) in mat4 inverse_model_matrix;
layout (location=10) in vec3 colour;
layout (location=11) in float metallic;
layout (location=12) in float roughness;

layout(set=0, binding=0) uniform UniformBufferObject {
  mat4 view_matrix;
  mat4 projection_matrix;
} ubo;

layout (location=0) out vec3 fragColor;
layout (location=1) out vec3 fragNormal;
layout (location=2) out vec4 fragWorldPos;
layout (location=3) out vec3 cameraPos;
layout (location=4) out float fragMetallic;
layout (location=5) out float fragRoughness;

void main() {
  fragWorldPos = model_matrix * vec4(position,1.0);
  gl_Position = ubo.projection_matrix * ubo.view_matrix * model_matrix * vec4(position,1.0);
  fragColor = colour;
  fragNormal = transpose(mat3(inverse_model_matrix)) * normal;
  fragMetallic = metallic;
  fragRoughness = roughness;

  cameraPos =
    - ubo.view_matrix[3][0] * vec3(ubo.view_matrix[0][0], ubo.view_matrix[1][0], ubo.view_matrix[2][0])
    - ubo.view_matrix[3][1] * vec3(ubo.view_matrix[0][1], ubo.view_matrix[1][1], ubo.view_matrix[2][1])
    - ubo.view_matrix[3][2] * vec3(ubo.view_matrix[0][2], ubo.view_matrix[1][2], ubo.view_matrix[2][2]);
}