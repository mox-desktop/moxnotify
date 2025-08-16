struct ProjectionUniform {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> projection: ProjectionUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) rect_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_size: vec4<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) scale: f32,
    @location(8) depth: f32,
};

struct InstanceInput {
    @location(1) rect_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_size: vec4<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) scale: f32,
    @location(8) depth: f32,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let position = model.position * (instance.rect_size + vec2<f32>(instance.border_size[0], instance.border_size[2]) + vec2<f32>(instance.border_size[1], instance.border_size[3])) * instance.scale + instance.rect_pos * instance.scale;
    out.clip_position = projection.projection * vec4<f32>(position, instance.depth, 1.0);
    out.uv = position;
    out.rect_pos = (instance.rect_pos + vec2<f32>(instance.border_size[0], instance.border_size[2])) * instance.scale;
    out.rect_size = instance.rect_size * instance.scale;
    out.rect_color = instance.rect_color;

    let outer_max_radius = min(
        instance.rect_size.x + instance.border_size[0] + instance.border_size[1],
        instance.rect_size.y + instance.border_size[2] + instance.border_size[3],
    ) * 0.5;

    out.border_radius = vec4<f32>(
        min(instance.border_radius[0] + instance.border_size[0] + instance.border_size[2], outer_max_radius),
        min(instance.border_radius[1] + instance.border_size[1] + instance.border_size[2], outer_max_radius),
        min(instance.border_radius[2] + instance.border_size[0] + instance.border_size[3], outer_max_radius),
        min(instance.border_radius[3] + instance.border_size[1] + instance.border_size[3], outer_max_radius)
    ) * instance.scale;

    out.border_size = instance.border_size * instance.scale;
    out.border_color = instance.border_color;
    out.scale = instance.scale;
    out.depth = instance.depth;

    return out;
}

fn sdf_rounded_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var x = r.x;
    var y = r.y;
    x = select(r.z, r.x, p.x > 0.0);
    y = select(r.w, r.y, p.x > 0.0);
    x = select(y, x, p.y > 0.0);
    let q = abs(p) - b + x;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - x;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    var result = vec3<f32>(0.0);
    for (var i = 0; i < 3; i = i + 1) {
        if c[i] <= 0.04045 {
            result[i] = c[i] / 12.92;
        } else {
            result[i] = pow((c[i] + 0.055) / 1.055, 2.4);
        }
    }
    return result;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let inner_center = in.rect_pos + in.rect_size / 2.0;
    let inner_dist = sdf_rounded_rect(in.uv - inner_center, in.rect_size / 2.0, in.border_radius);

    let outer_size = in.rect_size + vec2<f32>(in.border_size[0], in.border_size[2]) + vec2<f32>(in.border_size[1], in.border_size[3]);
    let outer_center = in.rect_pos - vec2<f32>(in.border_size[0], in.border_size[2]) + outer_size / 2.0;
    let outer_dist = sdf_rounded_rect(in.uv - outer_center, outer_size / 2.0, in.border_radius);

    let inner_aa = fwidth(inner_dist);
    let outer_aa = fwidth(outer_dist);

    let inner_alpha = smoothstep(-inner_aa, inner_aa, -inner_dist);
    let outer_alpha = smoothstep(-outer_aa, outer_aa, -outer_dist);
    let border_alpha = outer_alpha - inner_alpha;

    // sRGB to linear + premultiply
    let inner_rgb = srgb_to_linear(in.rect_color.rgb);
    let inner_color = vec4<f32>(inner_rgb * in.rect_color.a, in.rect_color.a) * inner_alpha;

    let border_rgb = srgb_to_linear(in.border_color.rgb);
    let border_color = vec4<f32>(border_rgb * in.border_color.a, in.border_color.a) * border_alpha;

    var out: FragmentOutput;
    out.color = inner_color + border_color;
    out.depth = in.clip_position.z / in.clip_position.w;
    return out;
}

