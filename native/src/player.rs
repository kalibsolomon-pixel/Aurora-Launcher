//! Small native software player renderer. No GPU, engine, timer or image decoder.
//! Geometry follows the existing launcher playerModel contract, in Minecraft texels.
//! Textures arrive already decoded/validated by the cosmetics service.
use crate::api::{HeadAvatar, Texture};
const W: usize = 320;
const H: usize = 440;
pub const WIDTH: u32 = W as u32;
pub const HEIGHT: u32 = H as u32;
pub const DEFAULT_YAW: f32 = -0.42;
#[derive(Clone, Copy, Default, Debug)]
struct P {
    x: f32,
    y: f32,
    z: f32,
}
impl P {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}
struct Face {
    p: [P; 4],
    uv: [usize; 4],
    mirror: bool,
    shade: f32,
    cape: bool,
}
struct Mesh {
    faces: Vec<Face>,
}
impl Mesh {
    #[allow(clippy::too_many_arguments)]
    fn cuboid(
        &mut self,
        w: f32,
        h: f32,
        d: f32,
        c: P,
        u: usize,
        v: usize,
        pitch: f32,
        yaw: f32,
        mirror: bool,
        grow: f32,
        cape: bool,
    ) {
        let (a, b) = (pitch.to_radians(), yaw.to_radians());
        let point = |x: f32, y: f32, z: f32| {
            let py = y * a.cos() - z * a.sin();
            let pz = y * a.sin() + z * a.cos();
            P::new(
                x * b.cos() + pz * b.sin() + c.x,
                py + c.y,
                -x * b.sin() + pz * b.cos() + c.z,
            )
        };
        let (x, z, t, bot) = (w / 2. + grow, d / 2. + grow, -grow, h + grow);
        let (w, h, d) = (w as usize, h as usize, d as usize);
        let mut q = |p, uv, shade| {
            self.faces.push(Face {
                p,
                uv,
                mirror,
                shade,
                cape,
            })
        };
        q(
            [
                point(-x, t, -z),
                point(x, t, -z),
                point(x, bot, -z),
                point(-x, bot, -z),
            ],
            [u + d, v + d, w, h],
            1.,
        );
        q(
            [
                point(x, t, z),
                point(-x, t, z),
                point(-x, bot, z),
                point(x, bot, z),
            ],
            [u + d + w + d, v + d, w, h],
            0.72,
        );
        q(
            [
                point(-x, t, z),
                point(-x, t, -z),
                point(-x, bot, -z),
                point(-x, bot, z),
            ],
            [if mirror { u + d + w } else { u }, v + d, d, h],
            0.78,
        );
        q(
            [
                point(x, t, -z),
                point(x, t, z),
                point(x, bot, z),
                point(x, bot, -z),
            ],
            [if mirror { u } else { u + d + w }, v + d, d, h],
            0.88,
        );
        q(
            [
                point(-x, t, z),
                point(x, t, z),
                point(x, t, -z),
                point(-x, t, -z),
            ],
            [u + d, v, w, d],
            1.04,
        );
        q(
            [
                point(-x, bot, -z),
                point(x, bot, -z),
                point(x, bot, z),
                point(-x, bot, z),
            ],
            [u + d + w, v, w, d],
            0.65,
        );
    }
}
fn valid_skin(s: &HeadAvatar) -> bool {
    matches!(s.skin_height, 32 | 64)
        && matches!(s.model.as_str(), "classic" | "slim")
        && s.skin_rgba.len() == 64 * s.skin_height as usize * 4
}
fn valid_cape(t: &Texture) -> bool {
    matches!((t.width, t.height), (64, 32) | (64, 64) | (128, 64))
        && t.rgba.len() == (t.width * t.height * 4) as usize
}
fn mesh(modern: bool, slim: bool, layers: bool, cape: bool) -> Mesh {
    let mut m = Mesh {
        faces: Vec::with_capacity(78),
    };
    let arm = if slim && modern { 3. } else { 4. };
    m.cuboid(
        8.,
        8.,
        8.,
        P::new(0., -16., 0.),
        0,
        0,
        0.,
        -8.,
        false,
        0.,
        false,
    );
    m.cuboid(
        8.,
        12.,
        4.,
        P::new(0., -8., 0.),
        16,
        16,
        0.,
        0.,
        false,
        0.,
        false,
    );
    m.cuboid(
        arm,
        12.,
        4.,
        P::new(-4. - arm / 2., -8., 0.),
        40,
        16,
        10.,
        0.,
        false,
        0.,
        false,
    );
    m.cuboid(
        arm,
        12.,
        4.,
        P::new(4. + arm / 2., -8., 0.),
        if modern { 32 } else { 40 },
        if modern { 48 } else { 16 },
        -10.,
        0.,
        !modern,
        0.,
        false,
    );
    m.cuboid(
        4.,
        12.,
        4.,
        P::new(-2., 4., 0.),
        0,
        16,
        -8.,
        0.,
        false,
        0.,
        false,
    );
    m.cuboid(
        4.,
        12.,
        4.,
        P::new(2., 4., 0.),
        if modern { 16 } else { 0 },
        if modern { 48 } else { 16 },
        8.,
        0.,
        !modern,
        0.,
        false,
    );
    if layers {
        m.cuboid(
            8.,
            8.,
            8.,
            P::new(0., -16., 0.),
            32,
            0,
            0.,
            -8.,
            false,
            0.45,
            false,
        );
        if modern {
            m.cuboid(
                8.,
                12.,
                4.,
                P::new(0., -8., 0.),
                16,
                32,
                0.,
                0.,
                false,
                0.25,
                false,
            );
            m.cuboid(
                arm,
                12.,
                4.,
                P::new(-4. - arm / 2., -8., 0.),
                40,
                32,
                10.,
                0.,
                false,
                0.25,
                false,
            );
            m.cuboid(
                arm,
                12.,
                4.,
                P::new(4. + arm / 2., -8., 0.),
                48,
                48,
                -10.,
                0.,
                false,
                0.25,
                false,
            );
            m.cuboid(
                4.,
                12.,
                4.,
                P::new(-2., 4., 0.),
                0,
                32,
                -8.,
                0.,
                false,
                0.25,
                false,
            );
            m.cuboid(
                4.,
                12.,
                4.,
                P::new(2., 4., 0.),
                0,
                48,
                8.,
                0.,
                false,
                0.25,
                false,
            );
        }
    }
    if cape {
        m.cuboid(
            10.,
            16.,
            1.,
            P::new(0., -8., 3.),
            0,
            0,
            12.,
            0.,
            false,
            0.,
            true,
        );
    }
    m
}
#[derive(Clone, Copy)]
struct Projected {
    x: f32,
    y: f32,
    z: f32,
    inv: f32,
}
fn project(p: P, yaw: f32, pitch: f32, zoom: f32) -> Projected {
    let px = p.x * yaw.cos() + p.z * yaw.sin();
    let pz = -p.x * yaw.sin() + p.z * yaw.cos();
    let py = p.y * pitch.cos() - pz * pitch.sin();
    let z = p.y * pitch.sin() + pz * pitch.cos();
    let inv = 1. / (1. + z / 120.);
    let scale = H as f32 / 40. * zoom * inv;
    Projected {
        x: W as f32 / 2. + px * scale,
        y: H as f32 / 2. + py * scale,
        z,
        inv,
    }
}
fn edge(a: Projected, b: Projected, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}
fn top_left(a: Projected, b: Projected) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}

/// At most 78 quads and a fixed 320×440 output. Rotation/zoom inputs are bounded
/// again here, even though the controller clamps them. Opaque depth precedes
/// back-to-front transparent layers, with perspective-correct nearest texels.
pub fn render(
    skin: Option<&HeadAvatar>,
    cape: Option<&Texture>,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    layers: bool,
) -> Vec<u8> {
    let skin = skin.filter(|s| valid_skin(s));
    let cape = cape.filter(|c| valid_cape(c));
    let m = mesh(
        skin.is_none_or(|s| s.skin_height == 64),
        skin.is_some_and(|s| s.model == "slim"),
        layers && skin.is_some(),
        cape.is_some(),
    );
    let yaw = if yaw.is_finite() {
        yaw % std::f32::consts::TAU
    } else {
        DEFAULT_YAW
    };
    let pitch = if pitch.is_finite() {
        pitch.clamp(-0.6, 0.6)
    } else {
        0.1
    };
    let zoom = if zoom.is_finite() {
        zoom.clamp(0.65, 1.2)
    } else {
        1.
    };
    let mut faces = m
        .faces
        .iter()
        .filter_map(|f| {
            let p = f.p.map(|p| project(p, yaw, pitch, zoom));
            (edge(p[0], p[1], p[2].x, p[2].y) > 0.).then_some((f, p))
        })
        .collect::<Vec<_>>();
    faces.sort_by(|a, b| {
        b.1.iter()
            .map(|p| p.z)
            .sum::<f32>()
            .total_cmp(&a.1.iter().map(|p| p.z).sum::<f32>())
    });
    let mut out = vec![0u8; W * H * 4];
    let mut depth = vec![f32::INFINITY; W * H];
    for transparent in [false, true] {
        for (face, p) in &faces {
            let texture = if face.cape {
                cape.map(|c| (c.rgba.as_slice(), c.width as usize))
            } else {
                skin.map(|s| (s.skin_rgba.as_slice(), 64))
            };
            // The neutral fallback is explicit and has no invented account texture.
            let tex_scale = if face.cape {
                cape.map_or(1, |c| c.width as usize / 64)
            } else {
                1
            };
            let [u, v, tw, th] = face.uv.map(|v| v * tex_scale);
            // Most outer faces are entirely transparent and most base faces
            // entirely opaque. Classify their tiny texel rectangles before
            // walking screen pixels, so neither pass rasterizes empty work.
            let contributes = if let Some((bytes, width)) = texture {
                (v..v + th).any(|y| {
                    (u..u + tw).any(|x| {
                        let alpha = bytes.get((y * width + x) * 4 + 3).copied().unwrap_or(0);
                        if transparent {
                            alpha > 0 && alpha < 255
                        } else {
                            alpha == 255
                        }
                    })
                })
            } else {
                !transparent
            };
            if !contributes {
                continue;
            }
            let uv = [(0., 0.), (1., 0.), (1., 1.), (0., 1.)];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                let [a, b, c] = tri.map(|i| p[i]);
                let area = edge(a, b, c.x, c.y);
                if area < 0.001 {
                    continue;
                }
                let min_x = a.x.min(b.x).min(c.x).floor().max(0.) as usize;
                let max_x = a.x.max(b.x).max(c.x).ceil().min((W - 1) as f32) as usize;
                let min_y = a.y.min(b.y).min(c.y).floor().max(0.) as usize;
                let max_y = a.y.max(b.y).max(c.y).ceil().min((H - 1) as f32) as usize;
                for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        let fx = x as f32 + 0.5;
                        let fy = y as f32 + 0.5;
                        let e = [edge(b, c, fx, fy), edge(c, a, fx, fy), edge(a, b, fx, fy)];
                        let inclusive = [top_left(b, c), top_left(c, a), top_left(a, b)];
                        if (0..3).any(|i| e[i] < 0. || (e[i] == 0. && !inclusive[i])) {
                            continue;
                        }
                        let weights = [
                            e[0] / area * a.inv,
                            e[1] / area * b.inv,
                            e[2] / area * c.inv,
                        ];
                        let sum: f32 = weights.iter().sum();
                        let weights = weights.map(|v| v / sum);
                        let z = weights[0] * a.z + weights[1] * b.z + weights[2] * c.z;
                        let pixel = y * W + x;
                        if z > depth[pixel] + 0.0001 {
                            continue;
                        }
                        let mut tx = 0.;
                        let mut ty = 0.;
                        for i in 0..3 {
                            tx += weights[i] * uv[tri[i]].0;
                            ty += weights[i] * uv[tri[i]].1;
                        }
                        if face.mirror {
                            tx = 1. - tx;
                        }
                        let color = if let Some((bytes, width)) = texture {
                            let index = ((v
                                + (ty * th as f32).floor().clamp(0., (th - 1) as f32) as usize)
                                * width
                                + u
                                + (tx * tw as f32).floor().clamp(0., (tw - 1) as f32) as usize)
                                * 4;
                            if index + 4 > bytes.len() {
                                continue;
                            }
                            [
                                bytes[index],
                                bytes[index + 1],
                                bytes[index + 2],
                                bytes[index + 3],
                            ]
                        } else {
                            [137, 150, 169, 255]
                        };
                        if color[3] == 0 || (color[3] < 255) != transparent {
                            continue;
                        }
                        if !transparent {
                            depth[pixel] = z;
                        }
                        let dst = pixel * 4;
                        let alpha = color[3] as f32 / 255.;
                        let back = out[dst + 3] as f32 / 255.;
                        let total = alpha + back * (1. - alpha);

                        // Store straight RGBA; Slint performs the final composition.
                        for ch in 0..3 {
                            let source = (color[ch] as f32 * face.shade).min(255.);
                            out[dst + ch] = ((source * alpha
                                + out[dst + ch] as f32 * back * (1. - alpha))
                                / total)
                                .round()
                                .clamp(0., 255.) as u8;
                        }
                        out[dst + 3] = (total * 255.).round() as u8;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skin(model: &str, height: u32) -> HeadAvatar {
        HeadAvatar {
            rgba: vec![],
            skin_rgba: vec![255; 64 * height as usize * 4],
            skin_height: height,
            model: model.into(),
        }
    }
    #[test]
    fn meshes_cover_classic_slim_legacy_and_layers() {
        for (modern, slim) in [(true, false), (true, true), (false, false)] {
            for layer in [false, true] {
                let m = mesh(modern, slim, layer, true);
                for face in m.faces {
                    let [u, v, w, h] = face.uv;
                    assert!(u + w <= 64);
                    assert!(v + h <= if modern { 64 } else { 32 });
                }
            }
        }
        assert_eq!(mesh(true, true, true, true).faces.len(), 78);
        let classic = mesh(true, false, false, false);
        let slim = mesh(true, true, false, false);
        assert_ne!(classic.faces[12].p[0].x, slim.faces[12].p[0].x);
    }
    #[test]
    fn transparent_empty_and_invalid_input_are_bounded() {
        let mut s = skin("slim", 64);
        s.skin_rgba.fill(0);
        assert!(
            render(Some(&s), None, DEFAULT_YAW, 0.1, 1., true)
                .iter()
                .all(|v| *v == 0)
        );
        s.skin_rgba.pop();
        assert_eq!(
            render(Some(&s), None, f32::NAN, f32::INFINITY, 999., true).len(),
            W * H * 4
        );
    }
    #[test]
    fn rotation_layers_and_cape_change_real_geometry() {
        let s = skin("classic", 64);
        let a = render(Some(&s), None, DEFAULT_YAW, 0.1, 1., false);
        assert!(a.chunks_exact(4).filter(|p| p[3] > 0).count() > 15_000);
        assert_ne!(a, render(Some(&s), None, 1.5, 0.1, 1., false));
        let cape = Texture {
            width: 64,
            height: 32,
            rgba: vec![255; 64 * 32 * 4],
        };
        assert_ne!(
            render(Some(&s), None, 3., 0.1, 1., true),
            render(Some(&s), Some(&cape), 3., 0.1, 1., true)
        );
    }
    #[test]
    fn front_uvs_sample_the_head_and_torso_regions() {
        let mut s = skin("classic", 64);
        for y in 0..64 {
            for x in 0..64 {
                let i = (y * 64 + x) * 4;
                s.skin_rgba[i] = x as u8 * 4;
                s.skin_rgba[i + 1] = y as u8 * 4;
                s.skin_rgba[i + 2] = 0;
            }
        }
        let pixels = render(Some(&s), None, 0., 0., 1., false);
        let head = (90 * W + 160) * 4;
        assert!((44..=52).contains(&pixels[head]), "head u {}", pixels[head]);
        assert!(
            (44..=52).contains(&pixels[head + 1]),
            "head v {}",
            pixels[head + 1]
        );
        let torso = (198 * W + 160) * 4;
        assert!(
            (92..=100).contains(&pixels[torso]),
            "torso u {}",
            pixels[torso]
        );
        assert!(
            (100..=108).contains(&pixels[torso + 1]),
            "torso v {}",
            pixels[torso + 1]
        );
    }
    #[test]
    fn transparent_outer_layers_reveal_the_base_without_seams() {
        let mut s = skin("classic", 64);
        s.skin_rgba.fill(0);
        for y in 0..16 {
            for x in 0..32 {
                let i = (y * 64 + x) * 4;
                s.skin_rgba[i..i + 4].copy_from_slice(&[200, 0, 0, 255]);
            }
        }
        let base = render(Some(&s), None, 0., 0., 1., false);
        assert_eq!(base, render(Some(&s), None, 0., 0., 1., true));
        for y in 0..16 {
            for x in 32..64 {
                let i = (y * 64 + x) * 4;
                s.skin_rgba[i..i + 4].copy_from_slice(&[0, 0, 200, 128]);
            }
        }
        let layered = render(Some(&s), None, 0., 0., 1., true);
        let i = (90 * W + 160) * 4;
        assert!((90..=110).contains(&layered[i]));
        assert!((90..=110).contains(&layered[i + 2]));
        assert_eq!(layered[i + 3], 255);
    }
}
