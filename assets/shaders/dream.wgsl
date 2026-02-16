// DeepDream post-processing effect: yellow tint, procedural eyes, swirl tendrils,
// and chromatic aberration.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct DreamSettings {
    intensity: f32,
    time: f32,
    _align: f32,
    _align2: f32,
}

@group(0) @binding(2) var<uniform> settings: DreamSettings;

// --- Hash functions for procedural placement ---

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(q) * 43758.5453);
}

fn hash1(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// --- Effect 1: Yellow tint ---

fn apply_yellow_tint(color: vec3<f32>, intensity: f32) -> vec3<f32> {
    let golden = vec3<f32>(1.0, 0.9, 0.4);
    let tinted = color * mix(vec3<f32>(1.0), golden, 1.2);
    return mix(color, tinted, intensity);
}

// --- Effect 2: Chromatic aberration ---

fn apply_chromatic_aberration(uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    let dir = uv - vec2<f32>(0.5);
    let offset = intensity * 0.015;
    let r = textureSample(screen_texture, screen_sampler, uv + dir * offset).r;
    let g = textureSample(screen_texture, screen_sampler, uv).g;
    let b = textureSample(screen_texture, screen_sampler, uv - dir * offset).b;
    return vec3<f32>(r, g, b);
}

// --- Shared: eye grid lookup ---

struct EyeInfo {
    center: vec2<f32>,
    size: f32,
    cell_id: vec2<f32>,
}

const GRID_SIZE: f32 = 7.0;

fn get_eye(cell: vec2<f32>, aspect: f32, uv: vec2<f32>, time: f32) -> EyeInfo {
    let jitter = hash2(cell);
    let size = 0.015 + hash1(cell) * 0.012;

    // Drift up to one eye-width using per-eye phase offsets
    let phase = hash2(cell + vec2<f32>(99.0, 77.0)) * 6.28;
    let drift = vec2<f32>(sin(time * 0.4 + phase.x), sin(time * 0.3 + phase.y)) * size;

    let center = (cell + 0.3 + jitter * 0.4) / GRID_SIZE + drift;
    let diff = uv - center;
    let corrected = vec2<f32>(diff.x * aspect, diff.y);
    let dist = length(corrected);
    return EyeInfo(center, size, cell);
}

// --- Effect 3: Eye dots ---

fn eye_pattern(uv: vec2<f32>, intensity: f32, time: f32, aspect: f32) -> vec4<f32> {
    let cell = floor(uv * GRID_SIZE);

    var eye_color = vec3<f32>(0.0);
    var eye_alpha = 0.0;

    for (var dy = -1i; dy <= 1i; dy++) {
        for (var dx = -1i; dx <= 1i; dx++) {
            let neighbor = cell + vec2<f32>(f32(dx), f32(dy));
            let eye = get_eye(neighbor, aspect, uv, time);

            let diff = uv - eye.center;
            let corrected = vec2<f32>(diff.x * aspect, diff.y);
            let dist = length(corrected);

            let iris_outer = eye.size;
            let iris_inner = eye.size * 0.5;
            let pupil_base = eye.size * 0.25;

            if dist < iris_outer {
                // Pupil pulses slightly
                let pupil_radius = pupil_base * (1.0 + 0.15 * sin(time * 2.0 + hash1(neighbor) * 6.28));

                // Iris color varies per eye (amber to green)
                let hue_shift = hash1(neighbor + vec2<f32>(42.0, 0.0));
                let iris = mix(
                    vec3<f32>(0.4, 0.6, 0.1),
                    vec3<f32>(0.6, 0.4, 0.1),
                    hue_shift,
                );

                let iris_ring = smoothstep(iris_outer, iris_outer - 0.003, dist)
                              * smoothstep(iris_inner - 0.002, iris_inner, dist);
                let pupil = 1.0 - smoothstep(pupil_radius - 0.002, pupil_radius, dist);

                let this_color = mix(iris * iris_ring, vec3<f32>(0.02), pupil);
                let this_alpha = smoothstep(iris_outer, iris_outer - 0.003, dist);

                if this_alpha > eye_alpha {
                    eye_color = this_color;
                    eye_alpha = this_alpha;
                }
            }
        }
    }

    return vec4<f32>(eye_color, eye_alpha * intensity * 0.2);
}

// --- Effect 4: Swirl tendrils ---

fn swirl_pattern(uv: vec2<f32>, intensity: f32, time: f32, aspect: f32) -> f32 {
    let cell = floor(uv * GRID_SIZE);

    var swirl_accum = 0.0;

    for (var dy = -1i; dy <= 1i; dy++) {
        for (var dx = -1i; dx <= 1i; dx++) {
            let neighbor = cell + vec2<f32>(f32(dx), f32(dy));
            let eye = get_eye(neighbor, aspect, uv, time);

            let diff = uv - eye.center;
            let corrected = vec2<f32>(diff.x * aspect, diff.y);
            let dist = length(corrected);

            // Draw tendrils outside the eye, fading over distance
            let reach = eye.size * 3.0;
            if dist > eye.size && dist < reach {
                let angle = atan2(corrected.y, corrected.x);

                let curl_speed = 3.0 + hash1(neighbor + vec2<f32>(7.0, 0.0)) * 2.0;
                let curved_angle = angle + (dist - eye.size) * curl_speed + time * -0.3;

                let num_arms = 24.0 + floor(hash1(neighbor + vec2<f32>(13.0, 0.0)) * 8.0);
                let line_val = abs(sin(curved_angle * num_arms));
                let line = line_val * line_val * line_val * line_val
                         * line_val * line_val * line_val * line_val; // pow 8

                // Enclosing circles at inner and outer spiral boundaries
                let ring_width = 0.002;
                let inner_ring = smoothstep(eye.size - ring_width, eye.size, dist)
                               * (1.0 - smoothstep(eye.size, eye.size + ring_width, dist));
                let outer_ring = smoothstep(reach - ring_width, reach, dist)
                               * (1.0 - smoothstep(reach, reach + ring_width, dist))
                               * 0.2;

                let fade_out = 1.0 - smoothstep(eye.size, reach, dist);
                let fade_in = smoothstep(eye.size, eye.size * 1.3, dist);

                swirl_accum = max(swirl_accum, max(line * fade_out * fade_in, max(inner_ring, outer_ring))) * 0.2;
            }
        }
    }

    return swirl_accum * intensity;
}

// --- Compositing ---

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let intensity = settings.intensity;
    let time = settings.time;

    if intensity < 0.001 {
        return textureSample(screen_texture, screen_sampler, uv);
    }

    let dims = textureDimensions(screen_texture);
    let aspect = f32(dims.x) / f32(dims.y);

    // Staggered fade-in: effects layer in gradually
    let tint_i = smoothstep(0.0, 0.3, intensity);
    let aberr_i = smoothstep(0.1, 0.5, intensity);
    let swirl_i = smoothstep(0.4, 1.0, intensity) * 0.7;
    let eye_i = smoothstep(0.5, 1.0, intensity) * 0.7;

    // 1. Sample with chromatic aberration
    var color = apply_chromatic_aberration(uv, aberr_i);

    // 2. Yellow tint
    color = apply_yellow_tint(color, tint_i);

    // 3. Swirl tendrils (additive golden glow)
    let swirl = swirl_pattern(uv, swirl_i, time, aspect);
    let swirl_color = vec3<f32>(0.9, 0.7, 0.2) * swirl;
    color = color + swirl_color;

    // 4. Eye dots (alpha-blended on top)
    let eye = eye_pattern(uv, eye_i, time, aspect);
    color = mix(color, eye.rgb, eye.a);

    return vec4<f32>(color, 1.0);
}
