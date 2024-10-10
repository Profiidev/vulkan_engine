#version 450

layout (set=1, binding=0) buffer readonly StorageBufferObject {
  float num_directional_lights;
  float num_point_lights;
  vec3 data[];
} sbo;

layout (location = 0) out vec4 fragColor;

layout (location = 0) in vec3 fragColorIn;
layout (location = 1) in vec3 fragNormalIn;
layout (location = 2) in vec4 fragWorldPosIn;
layout (location = 3) in vec3 cameraPosIn;
layout (location = 4) in float metallic;
layout (location = 5) in float roughness;

struct DirectionalLight {
  vec3 direction_to_light;
  vec3 irradiance;
};

struct PointLight {
  vec3 position;
  vec3 irradiance;
};

const float PI = 3.14159265359;

float distribution(vec3 normal, vec3 half_vector, float roughness) {
  float ndoth = dot(normal, half_vector);
  if (ndoth > 0.0) {
    float r = roughness * roughness;
    return r / (PI * pow(1 + (r - 1) * ndoth * ndoth, 2));
  } else {
    return 0.0;
  }
}

float geometry(vec3 light_direction, vec3 normal, vec3 camera_direction, float roughness) {
  float ndotl = abs(dot(normal, light_direction));
  float ndotv = abs(dot(normal, camera_direction));
  return 0.5 / max(0.01, mix(2 * ndotl * ndotv, ndotl + ndotv, roughness));
}

vec3 compute_radiance(vec3 irradiance, vec3 light_direction, vec3 normal, vec3 camera_direction, vec3 color) {
  float ndotl = max(dot(normal, light_direction), 0.0);

  vec3 irradiance_on_surface = ndotl * irradiance;

  float roughness2 = roughness * roughness;

  vec3 f0 = mix(vec3(0.03),color,vec3(metallic));
  vec3 reflected_irradiance = (f0 + (1 - f0) * pow(1 - ndotl, 5)) * irradiance_on_surface;
  vec3 refracted_irradiance = irradiance_on_surface - reflected_irradiance;
  vec3 refracted_not_absorbed_irradiance = refracted_irradiance * (1 - metallic);
  
  vec3 half_vector = normalize((light_direction + camera_direction) / 2.0);
  float ndoth = max(dot(normal, half_vector), 0.0);
  vec3 F = f0 + (1 - f0) * pow(1 - ndoth, 5);
  vec3 relevant_reflection = reflected_irradiance * F * geometry(light_direction, normal, camera_direction, roughness2) * distribution(normal, half_vector, roughness2);
  return refracted_not_absorbed_irradiance * color / PI + relevant_reflection;
}

void main() {
  vec3 direction_to_camera = normalize(cameraPosIn - fragWorldPosIn.xyz);
  vec3 normal = normalize(fragNormalIn);
  vec3 l = vec3(0);

  int num_directional_lights = int(sbo.num_directional_lights);
  int num_point_lights = int(sbo.num_point_lights);

  for (int i = 0; i < num_directional_lights; i++) {
    vec3 data1 = sbo.data[i * 2];
    vec3 data2 = sbo.data[i * 2 + 1];
    DirectionalLight light = DirectionalLight(normalize(data1), data2);
    l += compute_radiance(light.irradiance, light.direction_to_light, normal, direction_to_camera, fragColorIn);
  }

  for(int i = 0; i < num_point_lights; i++){
    vec3 data1 = sbo.data[num_directional_lights * 2 + i * 2];
    vec3 data2 = sbo.data[num_directional_lights * 2 + i * 2 + 1];

    PointLight point_light = PointLight(data1, data2);
    vec3 to_light = normalize(point_light.position - fragWorldPosIn.xyz);
    float d = length(fragWorldPosIn.xyz - point_light.position);
    vec3 irradiance = point_light.irradiance / (4 * PI * d * d);

    l += compute_radiance(irradiance, to_light, normal, direction_to_camera, fragColorIn);
  }

  fragColor = vec4(l / (1 + l), 1.0);
  fragColor = vec4(1.0, 1.0, 1.0, 1.0);
}
