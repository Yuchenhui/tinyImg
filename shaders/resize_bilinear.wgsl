// 双线性插值缩放 compute shader
// TODO: 实现

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var tex_sampler: sampler;

struct Params {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
}

@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.dst_width || id.y >= params.dst_height) {
        return;
    }

    let uv = vec2<f32>(
        (f32(id.x) + 0.5) / f32(params.dst_width),
        (f32(id.y) + 0.5) / f32(params.dst_height),
    );

    let color = textureSampleLevel(input_tex, tex_sampler, uv, 0.0);
    textureStore(output_tex, vec2<i32>(i32(id.x), i32(id.y)), color);
}
