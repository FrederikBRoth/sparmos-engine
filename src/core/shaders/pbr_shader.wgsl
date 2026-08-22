const PI: f32 = 3.141592653589793;
@group(0) @binding(0)
var<storage, read> input: array<u32>;

@group(1) @binding(0)
var<storage, read_write> output: array<u32>;

@compute
@workgroup_size(64)
fn main(
    @builtin(global_invocation_id) global_invocation_id: vec3<u32>
) {

    output[0] = 32u;
    output[6] = 85u;
}

//Cook-Torrance BRDF(bidirectional reflective distribution function) 
//functions. BRDF scales incomming radiance based on the surfaces material proper
//ties
//
//Normal distribution function, D
fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, a: f32) -> f32 {
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    let nom = a2;
    let denom = (ndoth2 * (a2 - 1.0) + 1.0);
    let denom2 = PI * denom * denom;

    return nom / denom;
}

//Geometry function, G. Roughness essentially
fn geometry_schlick_ggx(ndotv: f32, k: f32) -> f32 {
    let nom = ndotv;
    let denom = ndotv * (1.0 - k) + k;

    return nom / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, k: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let ggx1 = geometry_schlick_ggx(ndotv, k);
    let ggx2 = geometry_schlick_ggx(ndotl, k);

    return ggx1 * ggx2;
}

//Fresnell, Fr. Essentially calculating the angly reflection for when you are looking 
//at material straight at it (90 degrees) or parallel

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}
