//! Recessed cove lighting and a dimensional, light-emitting wall inscription.
use super::*;
use details::tube;
mod lettering;

const SIGN_X: f32 = 4.425;
const SIGN_CENTER_Y: f32 = (1.4 + 3.29) * 0.5;
const SIGN_CENTER_Z: f32 = -1.5;
const SIGN_WIDTH: f32 = 2.40;
const CORE_RADIUS: f32 = 0.0058;
const GLASS_RADIUS: f32 = 0.0075;
const ROOM_LEFT: f32 = -4.4;
const ROOM_RIGHT: f32 = 4.5;
const ROOM_BACK: f32 = -4.5;
const ROOM_FRONT: f32 = 9.0;
const LED_HEIGHT: f32 = 3.565;

pub(super) fn build(scene: &mut Scene, plaster: usize) {
    cove(scene, plaster);
    inscription(scene);
}

fn perimeter(inset: f32, height: f32) -> [Vec3; 4] {
    [
        Vec3::new(ROOM_LEFT + inset, height, ROOM_BACK + inset),
        Vec3::new(ROOM_RIGHT - inset, height, ROOM_BACK + inset),
        Vec3::new(ROOM_RIGHT - inset, height, ROOM_FRONT - inset),
        Vec3::new(ROOM_LEFT + inset, height, ROOM_FRONT - inset),
    ]
}

fn ring_band(scene: &mut Scene, a: [Vec3; 4], b: [Vec3; 4], material: usize) {
    for side in 0..4 {
        let next = (side + 1) % 4;
        for points in [[a[side], a[next], b[next]], [a[side], b[next], b[side]]] {
            scene
                .primitives
                .push(Primitive::triangle(points, None, [Vec2::ZERO; 3], material));
        }
    }
}

fn cove(scene: &mut Scene, plaster: usize) {
    // Wall-mounted, mitered cornice: the open trough sits about 24 cm below
    // the ceiling. A raised front lip conceals the source from the room.
    let profile = [
        (0.0, 3.60),
        (0.0, 3.47),
        (0.018, 3.47),
        (0.028, 3.482),
        (0.048, 3.491),
        (0.065, 3.487),
        (0.084, 3.503),
        (0.102, 3.515),
        (0.140, 3.525),
        (0.180, 3.527),
        (0.205, 3.535),
        (0.220, 3.555),
        (0.220, 3.620),
        (0.202, 3.620),
        (0.190, 3.562),
        (0.034, 3.562),
        (0.034, 3.600),
    ];
    for i in 0..profile.len() {
        let a = profile[i];
        let b = profile[(i + 1) % profile.len()];
        ring_band(scene, perimeter(a.0, a.1), perimeter(b.0, b.1), plaster);
    }
    let aluminium = scene.add_material(Material::metal(Vec3::splat(0.66), 0.32));
    ring_band(
        scene,
        perimeter(0.060, LED_HEIGHT - 0.001),
        perimeter(0.109, LED_HEIGHT - 0.001),
        aluminium,
    );
    // Warm-white opal diffuser in the RGB scene model.
    // The opaque channel below blocks downward emission; this is a real emitter.
    let led = scene.add_material(Material::light(Vec3::new(22.0, 18.0, 13.0)));
    ring_band(
        scene,
        perimeter(0.098, LED_HEIGHT),
        perimeter(0.071, LED_HEIGHT),
        led,
    );
}

/// Place the visible bounds, including the swash, on the sideboard's centre line.
fn inscription_paths() -> Vec<Vec<Vec3>> {
    let strokes = lettering::strokes();
    let (lo, hi) = strokes.iter().flatten().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)),
    );
    let center = (lo + hi) * 0.5;
    let scale = SIGN_WIDTH / (hi.x - lo.x);
    let mut paths: Vec<Vec<Vec3>> = strokes
        .into_iter()
        .map(|stroke| {
            stroke
                .into_iter()
                .map(|p| {
                    let q = (p - center) * scale;
                    Vec3::new(SIGN_X, SIGN_CENTER_Y + q.y, SIGN_CENTER_Z + q.x)
                })
                .collect()
        })
        .collect();
    // Hand-bent script crosses itself in projection. Lift the later glass run
    // toward the room so crossing strokes are separate physical tubes.
    let mut previous: Vec<(Vec3, f32)> = Vec::new();
    let mut distance = 0.0;
    for path in &mut paths {
        distance += 1.0;
        let mut last = path[0];
        for point in path {
            distance += point.distance(last);
            last = *point;
            let mut x = SIGN_X;
            for &(p, d) in &previous {
                if distance - d < 0.075 {
                    continue;
                }
                let separation = Vec2::new(p.y - point.y, p.z - point.z).length();
                if separation < 0.038 {
                    let blend = (1.0 - (separation / 0.038).powi(2)).powi(2);
                    x = x.min(SIGN_X - (SIGN_X - p.x + 0.024) * blend);
                }
            }
            point.x = x;
            previous.push((*point, distance));
        }
    }
    paths
}

fn inscription(scene: &mut Scene) {
    let white = scene.add_material(Material::light(Vec3::splat(8.0)));
    // A thin-shell Fresnel jacket preserves direct light connections through the
    // envelope. It does not model cylindrical wall refraction or dispersion.
    let glass = scene.add_material(Material::thin_glass(1.52));
    let porcelain = scene.add_material(Material::matte(Vec3::new(0.80, 0.79, 0.74)));
    let nickel = scene.add_material(Material::metal(Vec3::splat(0.64), 0.24));
    let insulation = scene.add_material(Material::matte(Vec3::splat(0.045)));
    let fittings = Fittings {
        glass,
        porcelain,
        nickel,
        insulation,
    };
    for points in inscription_paths() {
        tube(scene, &points, CORE_RADIUS, white, 24);
        tube(scene, &points, GLASS_RADIUS, glass, 32);
        let mut since_support = 0.12;
        for pair in points.windows(2) {
            since_support += pair[0].distance(pair[1]);
            if since_support >= 0.24 {
                support(scene, pair[1], (pair[1] - pair[0]).normalize(), &fittings);
                since_support = 0.0;
            }
        }
        for (end, neighbor) in [(0, 1), (points.len() - 1, points.len() - 2)] {
            let p = points[end];
            scene.add_sphere(p, CORE_RADIUS, white);
            electrode(scene, p, (p - points[neighbor]).normalize(), &fittings);
        }
    }
    // The connections continue through the plaster into a recessed service box.
    scene.add_box(
        Vec3::new(4.535, 2.25, -1.64),
        Vec3::new(4.64, 2.43, -1.36),
        insulation,
    );
}

struct Fittings {
    glass: usize,
    porcelain: usize,
    nickel: usize,
    insulation: usize,
}

fn electrode(scene: &mut Scene, point: Vec3, tangent: Vec3, m: &Fittings) {
    let end = Vec3::new(4.472, point.y, point.z) + tangent * 0.025;
    let a = point;
    let b = point + tangent * 0.025;
    let c = end - Vec3::X * 0.025;
    let neck: Vec<_> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let s = 1.0 - t;
            a * s.powi(3) + b * (3.0 * s * s * t) + c * (3.0 * s * t * t) + end * t.powi(3)
        })
        .collect();
    tube(scene, &neck, GLASS_RADIUS, m.glass, 32);
    // An electrode sleeve, ceramic boot, and insulated return enter the wall.
    tube(scene, &[end - Vec3::X * 0.016, end], 0.0054, m.nickel, 24);
    tube(
        scene,
        &[end - Vec3::X * 0.004, end + Vec3::X * 0.012],
        0.009,
        m.porcelain,
        24,
    );
    scene.add_sphere(end + Vec3::X * 0.012, 0.009, m.porcelain);
    let feed = Vec3::new(4.553, end.y, end.z);
    tube(
        scene,
        &[
            end + Vec3::X * 0.012,
            feed,
            Vec3::new(4.553, SIGN_CENTER_Y, SIGN_CENTER_Z),
        ],
        0.0028,
        m.insulation,
        12,
    );
}

fn support(scene: &mut Scene, point: Vec3, tangent: Vec3, m: &Fittings) {
    let wall = Vec3::new(4.497, point.y, point.z);
    tube(
        scene,
        &[point + Vec3::X * 0.011, wall],
        0.0035,
        m.porcelain,
        16,
    );
    tube(scene, &[wall - Vec3::X * 0.003, wall], 0.012, m.nickel, 24);
    let across = Vec3::X.cross(tangent).normalize();
    let clip: Vec<_> = (0..=24)
        .map(|i| {
            let angle = -2.1 + 4.2 * i as f32 / 24.0;
            point + (Vec3::X * angle.cos() + across * angle.sin()) * (GLASS_RADIUS + 0.0007)
        })
        .collect();
    tube(scene, &clip, 0.0011, m.nickel, 8);
    for side in [-1.0, 1.0] {
        let screw = wall - Vec3::X * 0.004 + across * (side * 0.008);
        scene.add_sphere(screw, 0.003, m.nickel);
        scene.add_box(
            screw + Vec3::new(-0.0031, -0.0005, -0.002),
            screw + Vec3::new(-0.0027, 0.0005, 0.002),
            m.insulation,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Ray, offset};

    #[test]
    fn fixtures_follow_the_seating_and_sideboard_axes() {
        let layout = Layout::new(Lighting::NightNeon);
        assert!((layout.sofa_x - SOFA_PANEL_CENTER).abs() < 1e-6);
        assert!((layout.table_x - layout.sofa_x).abs() < 1e-6);
        assert!((layout.rug_center_x - layout.sofa_x).abs() < 1e-6);
        assert!((layout.pendant.x - layout.table_x).abs() < 1e-6);
        assert!((layout.pendant.z + 1.8).abs() < 1e-6);
        let corners = perimeter(0.0, LED_HEIGHT);
        assert_eq!(corners[0], Vec3::new(-4.4, LED_HEIGHT, -4.5));
        assert_eq!(corners[2], Vec3::new(4.5, LED_HEIGHT, 9.0));
        let paths = inscription_paths();
        let (lo, hi) = paths.iter().flatten().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        assert!(((lo.z + hi.z) * 0.5 + 1.5).abs() < 1e-5);
        assert!(((lo.y + hi.y) * 0.5 - SIGN_CENTER_Y).abs() < 1e-5);
        assert!(
            hi.z < -0.28 && lo.z > -2.72,
            "Sign fits above the sideboard"
        );
        assert!(lo.x > 4.25 && hi.x + GLASS_RADIUS < 4.5);
        for pair in paths.iter().flat_map(|path| path.windows(2)) {
            assert!(pair[0].is_finite() && pair[0].distance(pair[1]) > 1e-7);
        }
    }

    #[test]
    fn emitters_illuminate_the_wall_and_ceiling_through_open_channels() {
        let mut scene = Scene::empty(Camera::new(Vec3::ZERO, -Vec3::Z, 58.0, 1.0, 0.0, 1.0));
        let plaster = scene.add_material(Material::matte(Vec3::splat(0.8)));
        build(&mut scene, plaster);
        assert!(
            scene
                .primitives
                .iter()
                .all(|p| p.area().is_finite() && p.area() > 0.0)
        );
        scene.add_box(
            Vec3::new(-5.0, 3.8, -5.0),
            Vec3::new(5.0, 4.0, 10.0),
            plaster,
        );
        scene.add_box(Vec3::new(4.5, 0.0, -5.0), Vec3::new(4.7, 3.8, 5.0), plaster);
        scene.finish().unwrap();
        let mut ceiling_connections = 0;
        let mut wall_connections = 0;
        let mut visible_strips = 0;
        for light in &scene.lights {
            let primitive = &scene.primitives[light.primitive as usize];
            let (point, normal) = primitive.sample(&mut Rng::new(light.primitive, 0, 42));
            if point.y > 3.5 {
                let eye = Vec3::new(0.0, 1.6, 1.0);
                let direction = (eye - point).normalize();
                let origin = offset(point, normal, direction);
                if scene.shadow_transmittance(Ray { origin, direction }, origin.distance(eye)) > 0.0
                {
                    visible_strips += 1;
                }
            }
            let target = if point.y > 3.5 {
                Vec3::new(point.x, 3.799, point.z)
            } else {
                Vec3::new(4.499, point.y, point.z)
            };
            let direction = (target - point).normalize();
            if normal.dot(direction) <= 0.0 {
                continue;
            }
            let origin = offset(point, normal, direction);
            if scene
                .shadow_transmittance(Ray { origin, direction }, origin.distance(target) - 0.0001)
                > 0.5
            {
                if point.y > 3.5 {
                    ceiling_connections += 1;
                } else {
                    wall_connections += 1;
                }
            }
        }
        assert_eq!(
            ceiling_connections, 8,
            "All four LED runs must reach the ceiling"
        );
        assert_eq!(
            visible_strips, 0,
            "The cornice must conceal the LED sources from below"
        );
        assert!(
            wall_connections > 100,
            "Neon must cast real light onto its backing wall"
        );
    }
}
