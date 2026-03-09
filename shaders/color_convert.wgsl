// RGB ↔ YCbCr 色彩空间转换 compute shader
// TODO: 实现完整转换

@group(0) @binding(0) var<storage, read> input: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;

struct Params {
    pixel_count: u32,
    direction: u32,  // 0 = RGB→YCbCr, 1 = YCbCr→RGB
}

@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.pixel_count) {
        return;
    }

    let pixel = input[id.x];

    if (params.direction == 0u) {
        // RGB → YCbCr (BT.601)
        let y  =  0.299 * pixel.r + 0.587 * pixel.g + 0.114 * pixel.b;
        let cb = -0.169 * pixel.r - 0.331 * pixel.g + 0.500 * pixel.b + 0.5;
        let cr =  0.500 * pixel.r - 0.419 * pixel.g - 0.081 * pixel.b + 0.5;
        output[id.x] = vec4<f32>(y, cb, cr, pixel.a);
    } else {
        // YCbCr → RGB (BT.601)
        let y  = pixel.r;
        let cb = pixel.g - 0.5;
        let cr = pixel.b - 0.5;
        let r = y + 1.402 * cr;
        let g = y - 0.344 * cb - 0.714 * cr;
        let b = y + 1.772 * cb;
        output[id.x] = vec4<f32>(r, g, b, pixel.a);
    }
}
