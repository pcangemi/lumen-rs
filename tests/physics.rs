use glam::{Vec2, Vec3};
use lumen_rs::{
    bvh::{Bvh, box_hit},
    geometry::{Primitive, Ray},
    material::{Material, fresnel},
    render::{self, RenderSettings},
    rng::Rng,
    scene,
};

fn environment_fixture() -> scene::Scene {
    let mut scene = scene::preset("studio", 1.5).unwrap();
    scene.primitives.clear();
    scene.medium = None;
    scene.add_sphere(Vec3::X * 100.0, 1.0, 0);
    scene.camera = lumen_rs::camera::Camera::new(Vec3::ZERO, Vec3::Y, 90.0, 1.5, 0.0, 1.0);
    scene.textures = lumen_rs::texture::TextureAtlas {
        descriptors: vec![[0, 4, 2, 0]],
        pixels: vec![[4.0, 0.0, 0.0, 0.0]; 4]
            .into_iter()
            .chain(vec![[0.0, 2.0, 0.0, 0.0]; 4])
            .collect(),
    };
    scene.environment_map = Some(scene::EnvironmentMap {
        texture: 0,
        intensity: 0.5,
        rotation: 0.0,
        horizon_fade: 0.0,
    });
    scene.finish().unwrap();
    scene
}

#[test]
fn hdr_environment_preserves_radiance_and_clamps_poles() {
    let mut scene = environment_fixture();
    assert_eq!(scene.environment(Vec3::Y), Vec3::new(2.0, 0.0, 0.0));
    assert_eq!(scene.environment(-Vec3::Y), Vec3::new(0.0, 1.0, 0.0));
    let left = scene.environment(Vec3::new(-1.0, 0.3, 1e-6).normalize());
    let right = scene.environment(Vec3::new(-1.0, 0.3, -1e-6).normalize());
    assert!((left - right).length() < 1e-5);
    scene.environment_map.as_mut().unwrap().horizon_fade = 0.25;
    assert_eq!(scene.environment(-Vec3::Y), scene.horizon);
    assert_eq!(scene.environment(Vec3::X), scene.horizon);

    // Distinct longitude columns expose a mirrored map or the wrong rotation sign.
    for (i, texel) in scene.textures.pixels.iter_mut().enumerate() {
        *texel = [(i % 4) as f32, 0.0, 0.0, 0.0];
    }
    scene.environment_map.as_mut().unwrap().horizon_fade = 0.0;
    assert_eq!(scene.environment(Vec3::X).x, 0.75);
    assert_eq!(scene.environment(Vec3::Z).x, 1.25);
    scene.environment_map.as_mut().unwrap().rotation = std::f32::consts::FRAC_PI_2;
    assert_eq!(scene.environment(Vec3::X).x, 1.25);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a native Metal device"]
fn metal_hdr_environment_matches_cpu() {
    let mut scene = environment_fixture();
    for (i, texel) in scene.textures.pixels.iter_mut().enumerate() {
        texel[2] = (i % 4) as f32;
    }
    let map = scene.environment_map.as_mut().unwrap();
    map.rotation = 0.37;
    map.horizon_fade = 0.4;
    let settings = RenderSettings {
        width: 24,
        height: 16,
        samples: 8,
        max_depth: 4,
        ..Default::default()
    };
    let cpu = render::cpu(&scene, settings).unwrap();
    let gpu = lumen_rs::metal_backend::render(&scene, settings).unwrap();
    for (a, b) in cpu.pixels.iter().zip(&gpu.pixels) {
        assert!(
            (*a - *b).abs().max_element() < 0.0001,
            "HDR environment mismatch: {a:?} {b:?}"
        );
    }
    assert_eq!(gpu.stats.invalid_samples, 0);
}

#[test]
fn bvh_matches_brute_force_for_random_and_axis_aligned_rays() {
    let mut rng = Rng::new(1, 2, 3);
    let mut primitives = Vec::new();
    for _ in 0..200 {
        primitives.push(Primitive::sphere(
            rng.sphere() * rng.uniform() * 8.0,
            0.1 + rng.uniform(),
            0,
        ));
    }
    for _ in 0..100 {
        let p = rng.sphere() * 5.0;
        primitives.push(Primitive::triangle(
            [p, p + rng.sphere(), p + rng.sphere()],
            None,
            [Vec2::ZERO; 3],
            0,
        ));
    }
    let bvh = Bvh::build(&mut primitives);
    assert!(bvh.depth < 63);
    for i in 0..5000 {
        let ray = Ray {
            origin: rng.sphere() * 12.0,
            direction: if i % 10 == 0 { Vec3::X } else { rng.sphere() },
        };
        let fast = bvh.intersect(&primitives, ray, f32::INFINITY, false);
        let slow = primitives
            .iter()
            .enumerate()
            .filter_map(|(j, p)| p.intersect(ray, f32::INFINITY, j))
            .min_by(|a, b| a.t.total_cmp(&b.t));
        assert_eq!(fast.is_some(), slow.is_some());
        if let (Some(a), Some(b)) = (fast, slow) {
            assert!((a.t - b.t).abs() < 1e-5);
        }
    }
}
#[test]
fn parallel_slab_rays_are_robust_on_boundaries() {
    assert!(box_hit(
        Vec3::ZERO,
        Vec3::ONE,
        Ray {
            origin: Vec3::new(0.0, 0.5, -1.0),
            direction: Vec3::Z
        },
        100.0
    ));
    assert!(!box_hit(
        Vec3::ZERO,
        Vec3::ONE,
        Ray {
            origin: Vec3::new(-0.01, 0.5, -1.0),
            direction: Vec3::Z
        },
        100.0
    ));
}
#[test]
fn dielectric_fresnel_and_total_internal_reflection() {
    assert!((fresnel(1.0, 1.0, 1.5) - 0.04).abs() < 1e-6);
    assert_eq!(fresnel(0.5, 1.5, 1.0), 1.0);
    assert!(fresnel(0.01, 1.0, 1.5) > 0.94);
}
#[test]
fn sphere_exit_and_triangle_barycentrics() {
    let p = Primitive::sphere(Vec3::ZERO, 1.0, 0);
    let h = p
        .intersect(
            Ray {
                origin: Vec3::ZERO,
                direction: Vec3::X,
            },
            100.0,
            0,
        )
        .unwrap();
    assert!(!h.front);
    assert_eq!(h.t, 1.0);
    assert_eq!(h.geometric, -Vec3::X);
    let t = Primitive::triangle(
        [Vec3::ZERO, Vec3::X, Vec3::Y],
        None,
        [Vec2::ZERO, Vec2::X, Vec2::Y],
        0,
    );
    let h = t
        .intersect(
            Ray {
                origin: Vec3::new(0.25, 0.25, 1.0),
                direction: -Vec3::Z,
            },
            100.0,
            0,
        )
        .unwrap();
    assert!((h.uv - Vec2::splat(0.25)).length() < 1e-6);
    assert!(h.front);
}
#[test]
fn millimeter_spheres_retain_silhouettes_at_room_scale() {
    let radius = 0.0035;
    let center = Vec3::new(3.0, 0.04, -2.0);
    let sphere = Primitive::sphere(center, radius, 0);
    for direction in [
        Vec3::X,
        Vec3::Y,
        Vec3::Z,
        Vec3::new(1.0, 2.0, 3.0).normalize(),
    ] {
        let tangent = direction.any_orthonormal_vector();
        for distance in [3.0, 10.0, 30.0] {
            for offset in [0.0, 0.8, 1.2] {
                let ray = Ray {
                    origin: center - direction * distance + tangent * (radius * offset),
                    direction,
                };
                let hit = sphere.intersect(ray, 100.0, 0);
                if offset > 1.0 {
                    assert!(hit.is_none(), "Ray outside a wool bead must miss");
                } else {
                    let hit = hit.expect("Ray inside a wool bead silhouette must hit");
                    let expected = distance - radius * (1.0_f32 - offset * offset).sqrt();
                    assert!((hit.t - expected).abs() < 5e-6);
                    assert!(hit.uv.is_finite());
                    assert!((hit.geometric.length() - 1.0).abs() < 1e-5);
                    assert!(hit.front);
                }
            }
        }
        let exit = sphere
            .intersect(
                Ray {
                    origin: center,
                    direction,
                },
                1.0,
                0,
            )
            .unwrap();
        assert!(!exit.front);
        assert!((exit.t - radius).abs() < 1e-6);
    }
}

#[test]
fn shadow_connections_do_not_hit_a_small_emitter_before_their_endpoint() {
    use lumen_rs::geometry::offset;

    let emitter = Primitive::sphere(Vec3::new(0.2, 2.533, -1.35), 0.0055, 0);
    let mut visible_connections = 0;
    for origin in [Vec3::new(0.2, 0.11, -1.35), Vec3::new(1.0, 0.11, 0.6)] {
        for sample in 0..4096 {
            let (point, normal) = emitter.sample(&mut Rng::new(0, sample, 42));
            let direction = (point - origin).normalize();
            if normal.dot(-direction) <= 0.0 {
                continue;
            }
            let endpoint = offset(point, normal, -direction);
            let delta = endpoint - origin;
            let ray = Ray {
                origin,
                direction: delta.normalize(),
            };
            assert!(
                emitter.intersect(ray, delta.length() - 1e-5, 0).is_none(),
                "A visible bulb must not shadow itself before the sampled endpoint"
            );
            visible_connections += 1;
        }
    }
    assert!(visible_connections > 3000);
}

#[test]
fn ggx_is_finite_reciprocal_and_does_not_create_energy() {
    let mut rng = Rng::new(8, 9, 10);
    let n = Vec3::Y;
    let wo = Vec3::new(0.3, 0.95, 0.0).normalize();
    for roughness in [0.03, 0.2, 0.6, 1.0] {
        for metal in [false, true] {
            let mut m = if metal {
                Material::metal(Vec3::splat(0.8), roughness)
            } else {
                Material::matte(Vec3::splat(0.8))
            };
            m.surface[0] = roughness;
            let mut energy = Vec3::ZERO;
            for _ in 0..20000 {
                let wi = m.sample(n, wo, &mut rng);
                let (f, pdf) = m.eval(Vec3::splat(0.8), n, wo, wi);
                let (g, _) = m.eval(Vec3::splat(0.8), n, wi, wo);
                assert!(f.is_finite() && pdf.is_finite());
                assert!((f - g).length() < 1e-3 * (1.0 + f.length()));
                if pdf > 0.0 {
                    energy += f * (n.dot(wi).max(0.0) / pdf);
                }
            }
            energy /= 20000.0;
            assert!(
                energy.max_element() <= 1.04,
                "{roughness} {metal}: {energy}"
            );
        }
    }
}
#[test]
fn obj_import_render_is_finite_and_deterministic_across_threads() {
    let s = scene::preset("studio", 1.5).unwrap();
    assert!(s.primitives.len() > 3000);
    let cfg = RenderSettings {
        width: 24,
        height: 16,
        samples: 8,
        max_depth: 8,
        ..Default::default()
    };
    let a = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| render::cpu(&s, cfg).unwrap());
    let b = rayon::ThreadPoolBuilder::new()
        .num_threads(3)
        .build()
        .unwrap()
        .install(|| render::cpu(&s, cfg).unwrap());
    assert_eq!(a.pixels, b.pixels);
    assert_eq!(a.stats.invalid_samples, 0);
    assert!(a.pixels.iter().any(|p| p.max_element() > 0.1));
}
#[test]
fn constant_environment_is_preserved_through_clear_glass() {
    let mut s = scene::preset("studio", 1.0).unwrap();
    s.primitives = vec![Primitive::sphere(Vec3::ZERO, 1.0, 0)];
    s.materials = vec![Material::glass(1.5, Vec3::ZERO)];
    s.sky = Vec3::ONE;
    s.horizon = Vec3::ONE;
    s.finish().unwrap();
    let mut sum = Vec3::ZERO;
    for i in 0..20000 {
        let mut r = Rng::new(2, i, 42);
        sum += lumen_rs::integrator::trace(
            &s,
            Ray {
                origin: Vec3::new(0.0, 0.0, 3.0),
                direction: -Vec3::Z,
            },
            &mut r,
            32,
        );
    }
    let mean = sum / 20000.0;
    assert!((mean - Vec3::ONE).abs().max_element() < 0.02, "{mean}");
}
#[test]
fn invalid_settings_and_scenes_are_rejected() {
    assert!(
        RenderSettings {
            width: 0,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        RenderSettings {
            samples: 0,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        RenderSettings {
            max_depth: 65,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(scene::preset("unknown", 1.0).is_err());
}
#[test]
fn gpu_records_have_exact_msl_layout() {
    assert_eq!(std::mem::size_of::<Primitive>(), 144);
    assert_eq!(std::mem::size_of::<Material>(), 96);
    assert_eq!(std::mem::size_of::<lumen_rs::bvh::Node>(), 48);
    assert_eq!(std::mem::size_of::<lumen_rs::camera::Camera>(), 64);
    assert_eq!(std::mem::size_of::<lumen_rs::scene::Light>(), 16);
}
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal GPU accessible to the test process"]
fn metal_matches_cpu_statistics_and_batching() {
    for name in scene::PRESET_NAMES {
        let s = scene::preset(name, 1.5).unwrap();
        let cfg = RenderSettings {
            width: 73,
            height: 47,
            samples: 64,
            max_depth: 8,
            batch: 8,
            ..Default::default()
        };
        let cpu = render::cpu(&s, cfg).unwrap();
        let gpu = lumen_rs::metal_backend::render(&s, cfg).unwrap();
        let gpu2 =
            lumen_rs::metal_backend::render(&s, RenderSettings { batch: 32, ..cfg }).unwrap();
        let tiled =
            lumen_rs::metal_backend::render_progressive(&s, cfg, &Default::default(), |_| {})
                .unwrap();
        assert_eq!(gpu.stats.invalid_samples, 0);
        assert_eq!(gpu.pixels, gpu2.pixels);
        assert_eq!(gpu.pixels, tiled.pixels);
        let a: f32 = cpu
            .pixels
            .iter()
            .map(|c| lumen_rs::rng::luminance(*c))
            .sum();
        let b: f32 = gpu
            .pixels
            .iter()
            .map(|c| lumen_rs::rng::luminance(*c))
            .sum();
        assert!((a - b).abs() / a < 0.10, "{name}: CPU {a}, Metal {b}");
        println!("{name}: CPU luminance {a:.3}, Metal {b:.3}; batching exact");
    }
}

#[test]
fn closed_boxes_have_outward_normals_for_refraction() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    scene.add_box(-Vec3::ONE, Vec3::ONE, 0);
    scene.finish().unwrap();
    for axis in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
        let hit = scene
            .bvh
            .intersect(
                &scene.primitives,
                Ray {
                    origin: axis * 3.0,
                    direction: -axis,
                },
                100.0,
                false,
            )
            .unwrap();
        assert!(hit.front);
        assert!(hit.geometric.dot(axis) > 0.999);
    }
}

#[test]
fn progressive_tiles_match_offline_pixels_and_cover_partial_edges() {
    let scene = scene::preset("studio", 1.5).unwrap();
    let settings = RenderSettings {
        width: 73,
        height: 47,
        samples: 9,
        batch: 4,
        max_depth: 6,
        ..Default::default()
    };
    let offline = render::cpu(&scene, settings).unwrap();
    let mut updates = 0;
    let mut last_progress = 0.0;
    let progressive = lumen_rs::progressive::cpu(&scene, settings, &Default::default(), |update| {
        assert!(update.progress >= last_progress);
        assert_eq!(update.pixels.len(), update.tile.pixel_count());
        last_progress = update.progress;
        updates += 1;
    })
    .unwrap();
    assert_eq!(updates, 6);
    assert_eq!(last_progress, 1.0);
    assert_eq!(offline.pixels, progressive.pixels);
    assert_eq!(offline.samples, progressive.samples);
}

#[test]
fn progressive_cancellation_returns_a_valid_partial_film() {
    let scene = scene::preset("studio", 1.0).unwrap();
    let settings = RenderSettings {
        width: 128,
        height: 64,
        samples: 16,
        batch: 4,
        ..Default::default()
    };
    let control = lumen_rs::progressive::RenderControl::default();
    let result = lumen_rs::progressive::cpu(&scene, settings, &control, |_| {
        control
            .cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    })
    .unwrap();
    assert_eq!(
        result.samples.iter().map(|s| *s as u64).sum::<u64>(),
        64 * 64 * 4
    );
    assert!(result.pixels.iter().all(|pixel| pixel.is_finite()));
}

#[test]
fn texture_wrapping_and_bilinear_filtering_are_consistent() {
    let atlas = lumen_rs::texture::TextureAtlas {
        pixels: vec![
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0, 0.0],
        ],
        descriptors: vec![[0, 2, 2, 0]],
    };
    assert_eq!(atlas.sample(0, Vec2::new(0.25, 0.75)), Vec3::X);
    assert_eq!(atlas.sample(0, Vec2::new(0.25, 0.25)), Vec3::Z);
    assert!((atlas.sample(0, Vec2::splat(0.5)) - Vec3::splat(0.5)).length() < 1e-6);
    assert_eq!(atlas.sample(0, Vec2::new(-0.75, 1.75)), Vec3::X);
}

#[test]
fn detailed_interior_loads_textures_and_has_finite_geometry() {
    let scene = scene::preset("interior", 1.5).unwrap();
    assert!(scene.textures.descriptors.len() >= 4);
    assert!(scene.primitives.len() > 40000);
    assert!(
        scene
            .primitives
            .iter()
            .all(|p| p.area().is_finite() && p.area() > 0.0)
    );
    let mut raised_pile_hits = 0;
    for x in 0..32 {
        for z in 0..32 {
            let ray = Ray {
                origin: Vec3::new(0.2 + x as f32 * 0.018, 0.1, -2.05 + z as f32 * 0.016),
                direction: -Vec3::Y,
            };
            let hit = scene.intersect(ray, 0.1, false).unwrap();
            assert!(hit.p.y < 0.051, "Boucle pile must not become tall spikes");
            raised_pile_hits += usize::from(hit.p.y > 0.036);
        }
    }
    assert!(
        raised_pile_hits > 20,
        "The open space between the table supports must retain full-height wool: {raised_pile_hits} raised hits"
    );
    let mut compressed_beads = 0;
    for primitive in &scene.primitives {
        if primitive.info[0] != 0 || scene.materials[primitive.info[1] as usize].detail[3] != -7.0 {
            continue;
        }
        let (lo, hi) = primitive.bounds();
        if lo.z > -1.403_161 || hi.z < -2.196_839 {
            continue;
        }
        for (left, right) in [(-0.299_217, -0.170_528), (1.170_528, 1.299_217)] {
            if lo.x <= right && hi.x >= left {
                assert!(hi.y < 0.036, "Compressed yarn must clear the table support");
                compressed_beads += 1;
            }
        }
    }
    assert!(compressed_beads > 100);
}

#[test]
fn absorption_matches_beer_lambert_through_a_closed_slab() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    scene.sky = Vec3::ONE;
    scene.horizon = Vec3::ONE;
    let absorption = Vec3::new(0.2, 0.5, 0.9);
    let material = scene.add_material(Material::glass(1.0, absorption));
    scene.add_box(-Vec3::ONE, Vec3::ONE, material);
    scene.finish().unwrap();
    let color = lumen_rs::integrator::trace(
        &scene,
        Ray {
            origin: Vec3::new(0.0, 0.0, 3.0),
            direction: -Vec3::Z,
        },
        &mut Rng::new(0, 0, 42),
        8,
    );
    assert!((color - (-absorption * 2.0).exp()).abs().max_element() < 3e-5);
}

#[test]
fn water_has_outward_boundaries_and_time_varying_surface() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    let water = scene.add_material(Material::glass(1.333, Vec3::splat(0.1)));
    let waves = scene::Waves {
        resolution: 16,
        ..Default::default()
    };
    scene
        .add_water(-Vec3::ONE, Vec3::ONE, water, waves)
        .unwrap();
    scene.finish().unwrap();
    for direction in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        let hit = scene
            .bvh
            .intersect(
                &scene.primitives,
                Ray {
                    origin: direction * 3.0,
                    direction: -direction,
                },
                10.0,
                false,
            )
            .unwrap();
        assert!(hit.front);
        assert!(hit.geometric.dot(direction) > 0.95);
        let exit = scene
            .bvh
            .intersect(
                &scene.primitives,
                Ray {
                    origin: Vec3::ZERO,
                    direction,
                },
                10.0,
                false,
            )
            .unwrap();
        assert!(!exit.front);
    }
    let height = |scene: &scene::Scene| {
        scene
            .bvh
            .intersect(
                &scene.primitives,
                Ray {
                    origin: Vec3::new(0.31, 3.0, 0.13),
                    direction: -Vec3::Y,
                },
                10.0,
                false,
            )
            .unwrap()
            .p
            .y
    };
    let first = height(&scene);
    scene.primitives.clear();
    scene
        .add_water(
            -Vec3::ONE,
            Vec3::ONE,
            water,
            scene::Waves { time: 1.0, ..waves },
        )
        .unwrap();
    scene.finish().unwrap();
    assert!((first - height(&scene)).abs() > 0.0001);
    assert!(
        scene
            .add_water(
                -Vec3::ONE,
                Vec3::ONE,
                water,
                scene::Waves {
                    resolution: 0,
                    ..waves
                }
            )
            .is_err()
    );
}

#[test]
fn volume_tracking_matches_analytic_and_numerical_transmittance() {
    use lumen_rs::medium::Medium;
    let mut rng = Rng::new(0, 0, 917);
    let ray = Ray {
        origin: Vec3::new(0.1, 0.2, -2.0),
        direction: Vec3::Z,
    };
    let medium = Medium::new(-Vec3::ONE, Vec3::ONE, 0.7, Vec3::splat(0.8), 0.6, 0.0).unwrap();
    let expected = (-1.4f32).exp();
    assert!((medium.transmittance(ray, 10.0, &mut rng) - expected).abs() < 1e-6);
    let trials = 40000;
    let escapes = (0..trials)
        .filter(|_| medium.sample(ray, 10.0, &mut rng).is_none())
        .count();
    assert!((escapes as f32 / trials as f32 - expected).abs() < 0.012);
    let mean_cosine = (0..trials)
        .map(|_| medium.sample_phase(Vec3::Z, &mut rng).z)
        .sum::<f32>()
        / trials as f32;
    assert!((mean_cosine - 0.6).abs() < 0.012);
    let cloud = Medium::new(-Vec3::ONE, Vec3::ONE, 2.0, Vec3::ONE, 0.0, 2.0).unwrap();
    let steps = 4096;
    let optical_depth: f32 = (0..steps)
        .map(|step| {
            let density = cloud.density(ray.at(1.0 + (step as f32 + 0.5) * 2.0 / steps as f32));
            assert!((0.0..=1.0).contains(&density));
            density * 4.0 / steps as f32
        })
        .sum();
    let estimate = (0..trials)
        .map(|_| cloud.transmittance(ray, 10.0, &mut rng))
        .sum::<f32>()
        / trials as f32;
    assert!((estimate - (-optical_depth).exp()).abs() < 0.012);
    assert_eq!(std::mem::size_of::<Medium>(), 64);
}

#[test]
fn incense_tracking_matches_integrated_density_and_stays_bounded() {
    use lumen_rs::medium::Medium;
    let source = Vec3::new(0.2, 0.3, -0.1);
    let height = 0.78;
    let smoke = Medium::incense(source, height, 75.0).unwrap();
    assert!(Medium::incense(source, 0.0, 75.0).is_err());
    assert!(Medium::incense(source, height, f32::NAN).is_err());
    assert_eq!(smoke.density(source - Vec3::Y * 0.001), 0.0);
    assert_eq!(smoke.density(source + Vec3::Y * height), 0.0);
    assert_eq!(smoke.density(source + Vec3::X * height), 0.0);
    let mut rng = Rng::new(0, 0, 1928);
    for _ in 0..20000 {
        let p = source
            + Vec3::new(
                (rng.uniform() - 0.5) * 0.56,
                rng.uniform(),
                (rng.uniform() - 0.5) * 0.56,
            ) * height;
        assert!((0.0..=1.0).contains(&smoke.density(p)));
    }
    // Find the strongest horizontal chord at several heights independently of
    // the procedural centre line, then integrate its extinction numerically.
    for fraction in [0.02, 0.35, 0.70] {
        let length = height * 0.56;
        let steps = 2048;
        let integrate = |ray: Ray| -> f32 {
            (0..steps)
                .map(|i| smoke.density(ray.at((i as f32 + 0.5) * length / steps as f32)))
                .sum::<f32>()
                * 75.0
                * length
                / steps as f32
        };
        let (ray, depth) = (0..129)
            .map(|i| {
                let ray = Ray {
                    origin: source
                        + Vec3::new(-0.28 + i as f32 * 0.56 / 128.0, fraction, -0.28) * height,
                    direction: Vec3::Z,
                };
                (ray, integrate(ray))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(
            depth > 0.02,
            "The plume must have a continuous visible stem"
        );
        let expected = (-depth).exp();
        let trials = 20000;
        let estimate: f32 = (0..trials)
            .map(|_| smoke.transmittance(ray, length, &mut rng))
            .sum::<f32>()
            / trials as f32;
        let escapes = (0..trials)
            .filter(|_| smoke.sample(ray, length, &mut rng).is_none())
            .count() as f32
            / trials as f32;
        assert!((estimate - expected).abs() < 0.012);
        assert!((escapes - expected).abs() < 0.012);
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal GPU accessible to the test process"]
fn metal_incense_attenuation_matches_cpu() {
    let mut s = scene::preset("studio", 1.5).unwrap();
    s.primitives.clear();
    s.sky = Vec3::ZERO;
    s.horizon = Vec3::ZERO;
    let light = s.add_material(Material::light(Vec3::ONE));
    s.add_quad(
        Vec3::new(-1.0, -1.0, -0.4),
        Vec3::X * 2.0,
        Vec3::Y * 2.0,
        light,
    );
    let mut smoke = lumen_rs::medium::Medium::incense(Vec3::ZERO, 0.78, 75.0).unwrap();
    // Absorption isolates the density field from phase and light sampling.
    smoke.scattering[..3].fill(0.0);
    s.medium = Some(smoke);
    s.camera = lumen_rs::camera::Camera::new(
        Vec3::new(0.02, 0.25, 0.5),
        Vec3::new(0.02, 0.25, 0.0),
        15.0,
        1.5,
        0.0,
        0.5,
    );
    s.finish().unwrap();
    let cfg = RenderSettings {
        width: 48,
        height: 32,
        samples: 512,
        max_depth: 2,
        ..Default::default()
    };
    let cpu = render::cpu(&s, cfg).unwrap();
    let gpu = lumen_rs::metal_backend::render(&s, cfg).unwrap();
    assert_eq!(gpu.stats.invalid_samples, 0);
    let cpu_loss: f32 = cpu.pixels.iter().map(|p| 1.0 - p.x).sum();
    let gpu_loss: f32 = gpu.pixels.iter().map(|p| 1.0 - p.x).sum();
    assert!(cpu_loss > 10.0);
    assert!((cpu_loss - gpu_loss).abs() / cpu_loss < 0.10);
    let error: f32 = cpu
        .pixels
        .iter()
        .zip(&gpu.pixels)
        .map(|(a, b)| (a.x - b.x).abs())
        .sum::<f32>()
        / cpu.pixels.len() as f32;
    assert!(
        error < 0.015,
        "Incense CPU/Metal mean absolute error: {error}"
    );
    println!("incense: CPU attenuation {cpu_loss:.3}, Metal {gpu_loss:.3}; MAE {error:.6}");
}

#[test]
fn small_obj_faces_survive_import_and_intersection() {
    let directory = std::env::temp_dir().join(format!("lumen-small-obj-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("tetrahedron.obj");
    std::fs::write(
        &path,
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nf 1 3 2\nf 1 2 4\nf 1 4 3\nf 2 3 4\n",
    )
    .unwrap();
    let mut s = scene::preset("studio", 1.0).unwrap();
    for scale in [0.00001, 0.0001, 0.001, 1.0] {
        s.primitives.clear();
        assert_eq!(
            s.import_obj(&path, Vec3::ZERO, scale, 0.0, None).unwrap(),
            4
        );
        s.finish().unwrap();
        let hit = s
            .intersect(
                Ray {
                    origin: Vec3::new(scale * 0.2, scale * 0.3, -1.0),
                    direction: Vec3::Z,
                },
                f32::INFINITY,
                false,
            )
            .unwrap();
        assert!(hit.front);
        assert!((hit.t - 1.0).abs() < 1e-6);
        assert_eq!(hit.geometric, -Vec3::Z);
        // A parallel ray must stay a miss even with a zero determinant scale.
        assert!(
            s.intersect(
                Ray {
                    origin: Vec3::new(0.0, 0.0, -1.0),
                    direction: Vec3::X
                },
                f32::INFINITY,
                false
            )
            .is_none()
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal GPU accessible to the test process"]
fn metal_preserves_small_triangle_visibility() {
    let mut s = scene::preset("studio", 1.0).unwrap();
    s.primitives.clear();
    s.sky = Vec3::ZERO;
    s.horizon = Vec3::ZERO;
    let radiance = Vec3::new(3.0, 2.0, 1.0);
    let light = s.add_material(Material::light(radiance));
    s.add_quad(
        Vec3::new(-0.000004, -0.000004, 0.0),
        Vec3::X * 0.000008,
        Vec3::Y * 0.000008,
        light,
    );
    s.camera = lumen_rs::camera::Camera::new(Vec3::Z, Vec3::ZERO, 0.0002, 1.4, 0.0, 1.0);
    s.finish().unwrap();
    let cfg = RenderSettings {
        width: 7,
        height: 5,
        samples: 4,
        max_depth: 1,
        ..Default::default()
    };
    let cpu = render::cpu(&s, cfg).unwrap();
    let gpu = lumen_rs::metal_backend::render(&s, cfg).unwrap();
    assert!(cpu.pixels.iter().all(|&p| p == radiance));
    assert_eq!(cpu.pixels, gpu.pixels);
    assert_eq!(gpu.stats.invalid_samples, 0);
}

#[test]
fn obj_textures_resolve_relative_to_nested_mtl_files() {
    let directory = std::env::temp_dir().join(format!("lumen-import-test-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("materials")).unwrap();
    image::RgbImage::from_pixel(2, 2, image::Rgb([128, 64, 32]))
        .save(directory.join("materials/color.png"))
        .unwrap();
    std::fs::write(
        directory.join("materials/surface.mtl"),
        "newmtl oak\nKd 1 1 1\nmap_Kd color.png\n",
    )
    .unwrap();
    std::fs::write(directory.join("mesh.obj"),
        "mtllib materials/surface.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl oak\nf 1/1 2/2 3/3\n").unwrap();
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene
        .import_obj(&directory.join("mesh.obj"), Vec3::ZERO, 1.0, 0.0, None)
        .unwrap();
    scene.finish().unwrap();
    assert_eq!(scene.textures.descriptors.len(), 1);
    let texel = scene.textures.sample(0, Vec2::ZERO);
    assert!((texel.x - 0.21586).abs() < 1e-4);
    assert!(
        scene
            .import_obj(&directory.join("mesh.obj"), Vec3::ZERO, 1.0, f32::NAN, None)
            .is_err()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn pbr_maps_preserve_linear_values_and_mirrored_tangent_frames() {
    let directory = std::env::temp_dir().join(format!("lumen-pbr-test-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("materials")).unwrap();
    image::RgbImage::from_pixel(2, 2, image::Rgb([192, 128, 238]))
        .save(directory.join("materials/normal.png"))
        .unwrap();
    image::RgbImage::from_pixel(2, 2, image::Rgb([128, 128, 128]))
        .save(directory.join("materials/rough.png"))
        .unwrap();
    std::fs::write(directory.join("materials/surface.mtl"), "newmtl cloth\nKd 1 1 1\nPr 0.8\nPm 0.5\nnorm normal.png\nmap_Pr rough.png\nmap_Pm rough.png\n").unwrap();
    std::fs::write(directory.join("mesh.obj"), "mtllib materials/surface.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl cloth\nf 1/1 2/2 3/3\n").unwrap();
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene
        .import_obj(&directory.join("mesh.obj"), Vec3::ZERO, 1.0, 0.0, None)
        .unwrap();
    let primitive = scene.primitives.len() - 1;
    let material = scene.primitives[primitive].info[1] as usize;
    let hit = lumen_rs::geometry::Hit {
        t: 1.0,
        p: Vec3::new(0.2, 0.2, 0.0),
        normal: Vec3::Z,
        geometric: Vec3::Z,
        uv: Vec2::splat(0.2),
        front: true,
        primitive,
        material,
    };
    let (mapped, normal) = scene.surface_shading(&hit, Vec3::Z);
    assert!((mapped.surface[0] - 0.8 * 128.0 / 255.0).abs() < 1e-6);
    assert!((mapped.surface[1] - 0.5 * 128.0 / 255.0).abs() < 1e-6);
    assert!(normal.x > 0.4 && normal.z > 0.7);
    scene.primitives[primitive].uv01[2] = -1.0;
    let (_, mirrored) = scene.surface_shading(&hit, Vec3::Z);
    assert!((mirrored.x + normal.x).abs() < 1e-5);
    assert!((mirrored.z - normal.z).abs() < 1e-5);
    scene.primitives[primitive].uv01 = [0.0; 4];
    scene.primitives[primitive].uv2 = [0.0; 4];
    assert_eq!(scene.surface_shading(&hit, Vec3::Z).1, Vec3::Z);
    scene.materials[material].maps[0] = u32::MAX;
    assert!(scene.finish().is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn alpha_cutouts_reject_camera_and_shadow_hits_without_hiding_the_back_surface() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    scene.materials.clear();
    scene.textures = lumen_rs::texture::TextureAtlas {
        pixels: vec![[0.0; 4], [1.0; 4]],
        descriptors: vec![[0, 2, 1, 0]],
    };
    let background = scene.add_material(Material::matte(Vec3::ONE));
    let mut mask = Material::matte(Vec3::ONE);
    mask.maps[3] = 1;
    mask.detail[3] = 1.0;
    let masked = scene.add_material(mask);
    // Back triangles precede the cutout, so a rejected nearer candidate must retain the accepted hit.
    scene.add_quad(
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::X * 2.0,
        Vec3::Y * 2.0,
        background,
    );
    scene.add_quad(
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::X * 2.0,
        Vec3::Y * 2.0,
        masked,
    );
    scene.finish().unwrap();
    let ray = |x| Ray {
        origin: Vec3::new(x, 0.0, 1.0),
        direction: -Vec3::Z,
    };
    assert_eq!(
        scene
            .intersect(ray(-0.5), f32::INFINITY, false)
            .unwrap()
            .material,
        background
    );
    assert_eq!(
        scene
            .intersect(ray(0.5), f32::INFINITY, false)
            .unwrap()
            .material,
        masked
    );
    assert!(scene.intersect(ray(-0.5), 1.5, true).is_none());
    assert!(scene.intersect(ray(0.5), 1.5, true).is_some());
}

#[test]
fn night_interior_uses_bulbs_without_daylight_emitters() {
    let scene = scene::preset("interior-night", 16.0 / 9.0).unwrap();
    assert!(scene.sky.max_element() < 0.02);
    let mut pendant = false;
    let mut arc_lamp = false;
    let mut table_lamp = false;
    let mut bulb_lo = Vec3::splat(f32::INFINITY);
    let mut bulb_hi = Vec3::splat(f32::NEG_INFINITY);
    for primitive in &scene.primitives {
        let (lo, hi) = primitive.bounds();
        if scene.materials[primitive.info[1] as usize].is_thin_glass()
            && ((lo + hi) * 0.5 - Vec3::new(0.2, 2.533, -1.35)).length() < 0.05
        {
            bulb_lo = bulb_lo.min(lo);
            bulb_hi = bulb_hi.max(hi);
        }
    }
    assert!(
        (0.079..0.081).contains(&(bulb_hi.x - bulb_lo.x)),
        "Preserve the full-size bulb envelope"
    );
    let mut street_lamps = 0;
    let mut exterior_ceiling_lights = 0;
    for light in &scene.lights {
        let primitive = &scene.primitives[light.primitive as usize];
        let (lo, hi) = primitive.bounds();
        let center = (lo + hi) * 0.5;
        let power = Vec3::from_array(
            scene.materials[primitive.info[1] as usize].emission[..3]
                .try_into()
                .unwrap(),
        );
        if center.x < -15.4 && primitive.info[0] == 1 {
            exterior_ceiling_lights += 1;
            assert!(
                scene
                    .intersect(
                        Ray {
                            origin: center - Vec3::Y * 0.002,
                            direction: -Vec3::Y,
                        },
                        0.3,
                        false,
                    )
                    .is_none(),
                "Apartment ceiling lights must have clear space beneath them: {center}"
            );
        }
        if (-9.0..-7.0).contains(&center.x) && center.y > 4.0 {
            assert!(
                power.max_element() <= 0.7,
                "Daylight must not leak into the night preset"
            );
        }
        pendant |=
            (center - Vec3::new(0.2, 2.50, -1.35)).length() < 0.1 && power.max_element() > 100.0;
        arc_lamp |= (center - Vec3::new(2.423, 1.777, -3.503)).length() < 0.1
            && power.max_element() > 100.0;
        table_lamp |=
            (center - Vec3::new(4.03, 1.059, -0.70)).length() < 0.04 && power.max_element() > 100.0;
        if center.x < -5.0 && center.y < 0.0 && power.max_element() > 100.0 {
            street_lamps += 1;
            let origin = Vec3::from_array(scene.camera.origin[..3].try_into().unwrap());
            let direction = (center - origin).normalize();
            let vector = |v: [f32; 4]| Vec3::from_array(v[..3].try_into().unwrap());
            let forward = direction.dot(vector(scene.camera.forward));
            let horizontal = direction.dot(vector(scene.camera.right));
            let vertical = direction.dot(vector(scene.camera.up));
            if forward > 0.0
                && horizontal.abs() <= forward * scene.camera.right[3]
                && vertical.abs() <= forward * scene.camera.up[3]
            {
                assert_eq!(
                    scene.shadow_transmittance(
                        Ray { origin, direction },
                        origin.distance(center) - 0.1
                    ),
                    0.0,
                    "Street luminaires inside the camera frustum must be occluded: {center}"
                );
            }
        }
    }
    assert!(
        pendant && arc_lamp && table_lamp,
        "All three bulb emitters must illuminate the night scene"
    );
    assert_eq!(
        street_lamps, 7,
        "The street must have working public lighting"
    );
    assert!(exterior_ceiling_lights > 0);
}

#[test]
fn thin_diffuse_shade_transmits_constant_radiance_from_either_side() {
    use lumen_rs::integrator::trace;
    let tint = Vec3::new(0.62, 0.71, 0.83);
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    scene.sky = Vec3::ONE;
    scene.horizon = Vec3::ONE;
    let shade = scene.add_material(Material::matte(tint).translucent(1.0));
    scene.add_quad(
        Vec3::new(-10.0, -10.0, 0.0),
        Vec3::X * 20.0,
        Vec3::Y * 20.0,
        shade,
    );
    scene.finish().unwrap();
    for side in [-1.0, 1.0] {
        for sample in 0..256 {
            let ray = Ray {
                origin: Vec3::Z * side,
                direction: -Vec3::Z * side,
            };
            let result = trace(&scene, ray, &mut Rng::new(0, sample, 42), 2);
            assert!((result - tint).abs().max_element() < 1e-5);
        }
    }
}

#[test]
fn thin_glass_shadows_accumulate_fresnel_losses_and_stop_at_opaque_geometry() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    let glass = scene.add_material(Material::thin_glass(1.5));
    for z in [0.0, 1.0] {
        scene.add_quad(
            Vec3::new(-2.0, -2.0, z),
            Vec3::X * 4.0,
            Vec3::Y * 4.0,
            glass,
        );
    }
    scene.finish().unwrap();
    let ray = Ray {
        origin: Vec3::new(0.2, 0.1, -1.0),
        direction: Vec3::Z,
    };
    let expected = (0.96_f32 / 1.04).powi(2);
    assert!((scene.shadow_transmittance(ray, 4.0) - expected).abs() < 1e-6);
    assert!(scene.intersect(ray, 4.0, false).is_some());
    let opaque = scene.add_material(Material::matte(Vec3::ONE));
    scene.add_quad(
        Vec3::new(-2.0, -2.0, 2.0),
        Vec3::X * 4.0,
        Vec3::Y * 4.0,
        opaque,
    );
    scene.finish().unwrap();
    assert_eq!(scene.shadow_transmittance(ray, 4.0), 0.0);
}

#[test]
fn thin_glass_light_connections_agree_with_pure_path_sampling() {
    let mut scene = scene::preset("studio", 1.0).unwrap();
    scene.primitives.clear();
    scene.sky = Vec3::ZERO;
    scene.horizon = Vec3::ZERO;
    let diffuse = scene.add_material(Material::matte(Vec3::splat(0.7)));
    let glass = scene.add_material(Material::thin_glass(1.52));
    let light = scene.add_material(Material::light(Vec3::splat(10.0)));
    for (z, material) in [(0.0, diffuse), (1.0, glass), (2.0, light)] {
        scene.add_quad(
            Vec3::new(-10.0, -10.0, z),
            Vec3::X * 20.0,
            Vec3::Y * 20.0,
            material,
        );
    }
    scene.finish().unwrap();
    let estimate = |scene: &scene::Scene| {
        let mut sum = 0.0;
        for sample in 0..32768 {
            let ray = Ray {
                origin: Vec3::new(0.2, 0.1, 0.5),
                direction: -Vec3::Z,
            };
            sum +=
                lumen_rs::integrator::trace(scene, ray, &mut Rng::new(0, sample, 42), 12).x as f64;
        }
        sum / 32768.0
    };
    let with_connections = estimate(&scene);
    scene.lights.clear();
    for primitive in &mut scene.primitives {
        primitive.info[2] = 0;
    }
    let pure_sampling = estimate(&scene);
    assert!(pure_sampling > 1.0);
    assert!(
        (with_connections - pure_sampling).abs() / pure_sampling < 0.03,
        "Thin-glass MIS mismatch: connections {with_connections}, pure paths {pure_sampling}"
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal GPU accessible to the test process"]
fn metal_offline_tiles_cover_odd_film_edges() {
    let scene = scene::preset("studio", 277.0 / 71.0).unwrap();
    let settings = RenderSettings {
        width: 277,
        height: 71,
        samples: 17,
        batch: 32,
        max_depth: 5,
        ..Default::default()
    };
    let offline = lumen_rs::metal_backend::render(&scene, settings).unwrap();
    let progressive = lumen_rs::metal_backend::render_progressive(
        &scene,
        RenderSettings {
            batch: 3,
            ..settings
        },
        &Default::default(),
        |_| {},
    )
    .unwrap();
    assert!(offline.samples.iter().all(|&count| count == 17));
    assert_eq!(offline.stats.invalid_samples, 0);
    assert_eq!(offline.pixels, progressive.pixels);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal GPU accessible to the test process"]
fn metal_adaptive_sampling_is_batch_invariant() {
    let scene = scene::preset("studio", 129.0 / 65.0).unwrap();
    let settings = RenderSettings {
        width: 129,
        height: 65,
        samples: 64,
        batch: 1,
        min_samples: 16,
        adaptive: 0.2,
        max_depth: 8,
        ..Default::default()
    };
    let single = lumen_rs::metal_backend::render(&scene, settings).unwrap();
    let batched = lumen_rs::metal_backend::render(
        &scene,
        RenderSettings {
            batch: 16,
            ..settings
        },
    )
    .unwrap();
    assert!(single.samples.iter().any(|&count| count < settings.samples));
    assert!(
        single
            .samples
            .iter()
            .all(|&count| count >= settings.min_samples)
    );
    assert_eq!(single.samples, batched.samples);
    assert_eq!(single.pixels, batched.pixels);
    assert_eq!(single.stats.invalid_samples, 0);
    assert_eq!(batched.stats.invalid_samples, 0);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a native Metal device"]
fn metal_resident_camera_reset_preserves_final_pixels() {
    use lumen_rs::{
        metal_backend::{MetalRenderer, MetalTuning},
        progressive::RenderControl,
    };
    let scene = lumen_rs::scene::preset("studio", 1.5).unwrap();
    let control = RenderControl::default();
    let settings = lumen_rs::render::RenderSettings {
        width: 48,
        height: 32,
        samples: 32,
        max_depth: 16,
        ..Default::default()
    };
    let baseline = lumen_rs::metal_backend::render(&scene, settings).unwrap();
    for tuning in [
        MetalTuning::default(),
        MetalTuning {
            compact_geometry: true,
            ..Default::default()
        },
        MetalTuning {
            specialize_scene: true,
            compact_stack: true,
            compact_geometry: false,
            threadgroup_limit: 128,
        },
        MetalTuning {
            specialize_scene: true,
            compact_stack: true,
            compact_geometry: false,
            threadgroup_limit: 64,
        },
    ] {
        let mut renderer = MetalRenderer::with_tuning(&scene, tuning).unwrap();
        let gpu_preview = renderer
            .preview(scene.camera, 48, 32, &control)
            .unwrap()
            .unwrap();
        let cpu_preview =
            lumen_rs::navigation::preview_cpu(&scene, scene.camera, 48, 32, &control).unwrap();
        let mean_error = gpu_preview
            .iter()
            .zip(&cpu_preview)
            .map(|(a, b)| (*a - *b).abs().max_element())
            .sum::<f32>()
            / gpu_preview.len() as f32;
        assert!(
            mean_error < 0.0001,
            "Preview CPU/Metal mismatch: {mean_error}"
        );
        let mut moved = scene.camera;
        moved.origin[0] += 0.4;
        renderer
            .render(moved, settings, &control, false, |_| {})
            .unwrap();
        let restored = renderer
            .render(scene.camera, settings, &control, false, |_| {})
            .unwrap();
        assert_eq!(
            restored.pixels, baseline.pixels,
            "Camera reset or compiler variant changed final pixels: {tuning:?}"
        );
        assert_eq!(restored.samples, baseline.samples);
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a native Metal device"]
fn metal_point_preview_preserves_depth_and_camera_orientation() {
    use lumen_rs::{
        camera::Camera,
        metal_points::PointRenderer,
        point_cloud::{Point, PointCloud},
    };
    let cloud = PointCloud {
        points: vec![
            Point {
                position: [0.0, 0.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Point {
                position: [0.0, 0.0, -1.0, 1.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
            Point {
                position: [0.0, 0.0, 4.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
        ],
    };
    let camera = Camera::new(
        glam::Vec3::new(0.0, 0.0, 3.0),
        glam::Vec3::ZERO,
        60.0,
        1.0,
        0.0,
        3.0,
    );
    let mut renderer = PointRenderer::new(&cloud).unwrap();
    let pixels = renderer.render(camera, 65, 65, 3).unwrap();
    let center = pixels[32 * 65 + 32];
    assert!(
        (center >> 16) > 150 && (center & 0xffff) == 0,
        "Front point must occlude the rear point: {center:06x}"
    );
    assert_eq!(pixels[0], 0x101921);
    let mut moved = camera;
    moved.origin[0] += 0.3;
    assert_ne!(pixels, renderer.render(moved, 65, 65, 3).unwrap());
    assert_eq!(pixels, renderer.render(camera, 65, 65, 3).unwrap());
}
