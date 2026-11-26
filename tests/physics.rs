use glam::{Vec2, Vec3};
use lumen_rs::{
    bvh::{Bvh, box_hit},
    geometry::{Primitive, Ray},
    material::{Material, fresnel},
    render::{self, RenderSettings},
    rng::Rng,
    scene,
};

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
    for light in &scene.lights {
        let primitive = &scene.primitives[light.primitive as usize];
        let (lo, hi) = primitive.bounds();
        let center = (lo + hi) * 0.5;
        let power = Vec3::from_array(
            scene.materials[primitive.info[1] as usize].emission[..3]
                .try_into()
                .unwrap(),
        );
        if center.x < -6.0 && center.y > 4.0 {
            assert!(
                power.max_element() <= 0.7,
                "Daylight must not leak into the night preset"
            );
        }
        pendant |=
            (center - Vec3::new(0.2, 2.50, -1.35)).length() < 0.1 && power.max_element() > 100.0;
        arc_lamp |= (center - Vec3::new(2.423, 1.777, -3.503)).length() < 0.1
            && power.max_element() > 100.0;
    }
    assert!(
        pendant && arc_lamp,
        "Both sourced bulb meshes must illuminate the night scene"
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
