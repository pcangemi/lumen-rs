//! Radiance transport: next-event estimation, MIS and nested dielectric media.
use crate::{
    geometry::{Ray, offset, unpack_vec3},
    material::fresnel,
    rng::{Rng, power},
    scene::Scene,
};
use glam::Vec3;

/// Trace one camera path in linear RGB. All directions must be normalized.
pub fn trace(scene: &Scene, mut ray: Ray, rng: &mut Rng, max_depth: u32) -> Vec3 {
    let mut radiance = Vec3::ZERO;
    let mut throughput = Vec3::ONE;
    let mut previous_pdf = 0.0;
    let mut previous_position = ray.origin;
    let mut previous_was_specular = true;
    // A stack supports properly nested, closed dielectric shells (air is implicit).
    let mut media = [0usize; 16];
    let mut medium_count = 0usize;
    let mut eta_scale = 1.0f32;
    for depth in 0..max_depth {
        let surface_hit = scene.intersect(ray, f32::INFINITY, false);
        let surface_distance = surface_hit.as_ref().map_or(f32::INFINITY, |hit| hit.t);
        if medium_count == 0
            && let Some(medium) = &scene.medium
            && let Some(distance) = medium.sample(ray, surface_distance, rng)
        {
            let position = ray.at(distance);
            throughput *= unpack_vec3(medium.scattering);
            radiance +=
                throughput * sample_volume_light(scene, medium, position, ray.direction, rng);
            let direction = medium.sample_phase(ray.direction, rng);
            previous_pdf = medium.phase(ray.direction.dot(direction));
            previous_position = position;
            previous_was_specular = false;
            ray = Ray {
                origin: position,
                direction,
            };
            if depth >= 3 {
                let survival = throughput.max_element().clamp(0.05, 0.95);
                if rng.uniform() > survival {
                    break;
                }
                throughput /= survival;
            }
            continue;
        }
        let Some(hit) = surface_hit else {
            radiance += throughput * scene.environment(ray.direction);
            break;
        };
        if medium_count > 0 {
            let absorb = unpack_vec3(scene.materials[media[medium_count - 1]].absorption);
            throughput *= (-absorb * hit.t).exp();
        }
        let (material, n) = scene.surface_shading(&hit, -ray.direction);
        if material.is_light() {
            let mut weight = 1.0;
            let lid = scene.primitives[hit.primitive].info[2];
            if !previous_was_specular && lid > 0 {
                let i = lid as usize - 1;
                let cos = hit.geometric.dot(-ray.direction).abs();
                let pdf = scene.light_probability(i) * hit.p.distance_squared(previous_position)
                    / (scene.lights[i].area * cos).max(1e-12);
                weight = power(previous_pdf, pdf);
            }
            radiance += throughput * unpack_vec3(material.emission) * weight;
            break;
        }
        let wo = -ray.direction;
        if material.is_glass() {
            let eta_i = if medium_count > 0 {
                scene.materials[media[medium_count - 1]].surface[3]
            } else {
                1.0
            };
            let eta_t = if hit.front {
                material.surface[3]
            } else if medium_count > 1 {
                scene.materials[media[medium_count - 2]].surface[3]
            } else {
                1.0
            };
            let cos = hit.geometric.dot(wo).clamp(0.0, 1.0);
            let f = fresnel(cos, eta_i, eta_t);
            let direction;
            if rng.uniform() < f {
                direction = ray.direction + 2.0 * cos * hit.geometric;
            } else {
                let eta = eta_i / eta_t;
                let perp = eta * (ray.direction + cos * hit.geometric);
                direction = perp - (1.0 - perp.length_squared()).max(0.0).sqrt() * hit.geometric;
                throughput *= eta * eta;
                eta_scale /= eta * eta;
                if hit.front {
                    if medium_count >= media.len() {
                        break;
                    }
                    media[medium_count] = hit.material;
                    medium_count += 1;
                } else {
                    medium_count = medium_count.saturating_sub(1);
                }
            }
            ray = Ray {
                origin: offset(hit.p, hit.geometric, direction),
                direction: direction.normalize(),
            };
            previous_was_specular = true;
        } else {
            let color = scene.surface_color(&hit);
            if !scene.lights.is_empty() {
                let pick = rng.uniform();
                let li = scene
                    .lights
                    .partition_point(|l| l.cdf < pick)
                    .min(scene.lights.len() - 1);
                let light = &scene.lights[li];
                let (lp, ln) = scene.primitives[light.primitive as usize].sample(rng);
                let to_light = lp - hit.p;
                let dist2 = to_light.length_squared();
                let dist = dist2.sqrt();
                let wi = to_light / dist;
                let cos_l = ln.dot(-wi).abs();
                let cos = n.dot(wi).max(0.0);
                if cos > 0.0 && cos_l > 1e-7 && hit.geometric.dot(wi) > 0.0 {
                    let pdf = scene.light_probability(li) * dist2 / (cos_l * light.area);
                    let (f, bsdf_pdf) = material.eval(color, n, wo, wi);
                    let origin = offset(hit.p, hit.geometric, wi);
                    let end = offset(lp, ln, -wi);
                    let shadow = end - origin;
                    let ray = Ray {
                        origin,
                        direction: shadow.normalize(),
                    };
                    if scene.intersect(ray, shadow.length() - 1e-5, true).is_none() {
                        let emission = unpack_vec3(
                            scene.materials
                                [scene.primitives[light.primitive as usize].info[1] as usize]
                                .emission,
                        );
                        let transmittance = scene.medium.as_ref().map_or(1.0, |medium| {
                            medium.transmittance(ray, shadow.length(), rng)
                        });
                        radiance += throughput
                            * transmittance
                            * f
                            * emission
                            * (cos * power(pdf, bsdf_pdf) / pdf.max(1e-20));
                    }
                }
            }
            let wi = material.sample(n, wo, rng);
            if wi.dot(hit.geometric) <= 0.0 {
                break;
            }
            let (f, pdf) = material.eval(color, n, wo, wi);
            if pdf <= 1e-12 {
                break;
            }
            throughput *= f * (n.dot(wi).max(0.0) / pdf);
            previous_pdf = pdf;
            previous_position = hit.p;
            previous_was_specular = false;
            ray = Ray {
                origin: offset(hit.p, hit.geometric, wi),
                direction: wi,
            };
        }
        if !throughput.is_finite() || throughput.max_element() <= 0.0 {
            break;
        }
        if depth >= 3 {
            let survival = (throughput.max_element() * eta_scale).clamp(0.05, 0.95);
            if rng.uniform() > survival {
                break;
            }
            throughput /= survival;
        }
    }
    radiance
}

/// Next-event estimation at a volume collision, including surface visibility and cloud extinction.
fn sample_volume_light(
    scene: &Scene,
    medium: &crate::medium::Medium,
    position: Vec3,
    forward: Vec3,
    rng: &mut Rng,
) -> Vec3 {
    if scene.lights.is_empty() {
        return Vec3::ZERO;
    }
    let choice = rng.uniform();
    let index = scene
        .lights
        .partition_point(|light| light.cdf < choice)
        .min(scene.lights.len() - 1);
    let light = &scene.lights[index];
    let primitive = &scene.primitives[light.primitive as usize];
    let (light_position, normal) = primitive.sample(rng);
    let difference = light_position - position;
    let distance_squared = difference.length_squared();
    let direction = difference.normalize();
    let cosine = normal.dot(-direction).abs();
    if cosine < 1e-7 {
        return Vec3::ZERO;
    }
    let pdf = scene.light_probability(index) * distance_squared / (cosine * light.area);
    let phase = medium.phase(forward.dot(direction));
    let target = offset(light_position, normal, -direction);
    let shadow = target - position;
    let ray = Ray {
        origin: position,
        direction: shadow.normalize(),
    };
    if scene.intersect(ray, shadow.length() - 1e-5, true).is_some() {
        return Vec3::ZERO;
    }
    let transmittance = medium.transmittance(ray, shadow.length(), rng);
    let emission = unpack_vec3(scene.materials[primitive.info[1] as usize].emission);
    emission * (transmittance * phase * power(pdf, phase) / pdf.max(1e-20))
}
