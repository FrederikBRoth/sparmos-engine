const PI: f32 = 3.141592653589793;
struct CameraUniform {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
}

struct LightBlock {
    lights: array<Light, 16>,
    light_count: u32,
}

@group(1) @binding(0)
var<uniform> u_lights: LightBlock;

@group(2) @binding(0)
var albedo_map: texture_2d<f32>;

@group(2) @binding(1)
var normal_map: texture_2d<f32>;

@group(2) @binding(2)
var metallic_map: texture_2d<f32>;
@group(2) @binding(3)
var roughness_map: texture_2d<f32>;
@group(2) @binding(4)
var ao_map: texture_2d<f32>;

@group(2) @binding(5)
var texture_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) texture: vec2<f32>,
    @location(2) normal: vec3<f32>,
}

struct InstanceInput {
    @location(5) pos_scale: vec4<f32>,
    @location(6) rotation: vec4<f32>,
    @location(7) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) uv: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let position = instance.pos_scale.xyz;
    let scale = instance.pos_scale.w;

    let rot = quat_to_mat3(instance.rotation);

    // Apply scale
    let rot_scaled = mat3x3<f32>(
        rot[0] * scale,
        rot[1] * scale,
        rot[2] * scale,
    );

    // Build full model matrix
    let model_matrix = mat4x4<f32>(
        vec4<f32>(rot_scaled[0], 0.0),
        vec4<f32>(rot_scaled[1], 0.0),
        vec4<f32>(rot_scaled[2], 0.0),
        vec4<f32>(position, 1.0),
    );

    let world_pos = model_matrix * vec4<f32>(model.position, 1.0);

    // Normal matrix = rotation only
    let normal = normalize(rot * model.normal);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.color = instance.color;
    out.world_normal = normal;
    out.world_position = world_pos.xyz;
    out.uv = model.texture;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = pow(
        textureSample(albedo_map, texture_sampler, in.uv).rgb,
        vec3<f32>(2.2)
    );

    let metallic = textureSample(
        metallic_map,
        texture_sampler,
        in.uv
    ).r;

    let roughness = textureSample(
        roughness_map,
        texture_sampler,
        in.uv
    ).r;

    let ao = textureSample(
        ao_map,
        texture_sampler,
        in.uv
    ).r;

    var N = get_normal_from_normal_map(
        in.uv,
        in.world_position,
        in.world_normal,
    );
    //N = normalize(in.world_normal);
    let V = normalize(camera.view_pos.xyz - in.world_position);

    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);
    var lo: vec3<f32> = vec3<f32>(0.0);

    for (var i: u32 = 0u; i < u_lights.light_count; i = i + 1u) {
        let light = u_lights.lights[i];

        let L = normalize(light.position - in.world_position);
        let H = normalize(V + L);

        let ndotl = max(dot(N, L), 0.0);
        let distance = length(light.position - in.world_position);
        let attenuation = 1.0 / (distance * distance);
        let radiance = light.color * light.intensity * attenuation;

        //full Cook-Torrance specular calculation
        //Surface reflection if looking directly at the material. This is different per material (think metals, plastic etc)
        //Should be changed based on that. we go for a constant
        let f = fresnel_schlick(max(dot(H, V), 0.0), f0);

        let ndf = distribution_ggx(N, H, roughness);
        let g = geometry_smith(N, V, L, roughness);

        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(N, V), 0.0) * ndotl + 0.0001;
        let specular = numerator / denominator;

        let kS = f;
        var kD = vec3<f32>(1.0) - kS;
        kD = kD * (1.0 - metallic);

        lo += (kD * albedo / PI + specular) * radiance * ndotl;
    }

    // Tone mapping
    let ambient = vec3<f32>(0.03) * ao * albedo;
    var color = ambient + lo;

    color = color / (color + vec3<f32>(1.0));
    //color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}

fn quat_to_mat3(q: vec4<f32>) -> mat3x3<f32> {
    let x = q.x;
    let y = q.y;
    let z = q.z;
    let w = q.w;

    return mat3x3<f32>(
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
    );
}

//Cook-Torrance BRDF(bidirectional reflective distribution function) 
//functions. BRDF scales incomming radiance based on the surfaces material proper
//ties
//
//Normal distribution function, D
fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    let nom = a2;
    var denom = (ndoth2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / denom;
}

//Geometry function, G. Roughness essentially
fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    let nom = ndotv;
    let denom = ndotv * (1.0 - k) + k;

    return nom / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let ggx1 = geometry_schlick_ggx(ndotv, roughness);
    let ggx2 = geometry_schlick_ggx(ndotl, roughness);

    return ggx1 * ggx2;
}

//Fresnell, Fr. Essentially calculating the angly reflection for when you are looking 
//at material straight at it (90 degrees) or parallel

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}
fn get_normal_from_normal_map(
    uv: vec2<f32>,
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
) -> vec3<f32> {
    let tangent_normal = textureSample(
        normal_map,
        texture_sampler,
        uv
    ).xyz * 2.0 - 1.0;

    let Q1 = dpdx(world_position);
    let Q2 = dpdy(world_position);

    let st1 = dpdx(uv);
    let st2 = dpdy(uv);

    let N = normalize(world_normal);

    let T = normalize(Q1 * st2.y - Q2 * st1.y);
    let B = -normalize(cross(N, T));

    let TBN = mat3x3<f32>(
        T,
        B,
        N
    );

    return normalize(TBN * tangent_normal);
}
