#include <metal_stdlib>
#if LUMEN_NATIVE_TRAVERSAL
#include <metal_raytracing>
#endif
using namespace metal;
#ifndef LUMEN_WIDE_BVH
#define LUMEN_WIDE_BVH 0
#endif
#ifndef LUMEN_BVH_STACK_SIZE
#define LUMEN_BVH_STACK_SIZE 64
#endif
constant bool compact_geometry_feature [[function_constant(2)]];
constant bool compact_geometry = is_function_constant_defined(compact_geometry_feature) ? compact_geometry_feature : false;
constant bool volume_feature [[function_constant(0)]];
constant bool opacity_feature [[function_constant(1)]];
constant bool has_volumes = is_function_constant_defined(volume_feature) ? volume_feature : true;
constant bool has_opacity = is_function_constant_defined(opacity_feature) ? opacity_feature : true;

#if LUMEN_NATIVE_TRAVERSAL
using namespace metal::raytracing;
struct NativeContext {
    primitive_acceleration_structure acceleration;
    intersection_function_table<> functions;
};
#define NATIVE_PARAMETER , NativeContext native
#define NATIVE_ARGUMENT , native
#define NATIVE_BINDINGS primitive_acceleration_structure acceleration [[buffer(10)]], intersection_function_table<> functions [[buffer(11)]],
#define NATIVE_CONTEXT NativeContext native = {acceleration, functions};
#else
#define NATIVE_PARAMETER
#define NATIVE_ARGUMENT
#define NATIVE_BINDINGS
#define NATIVE_CONTEXT
#endif

struct Primitive {
#if !LUMEN_SPLIT_GEOMETRY
    float4 p0, p1, p2;
#endif
    float4 n0, n1, n2;
#if LUMEN_DENSE_ATTRIBUTES
    float4 uv;
#else
    float4 uv01, uv2;
#endif
#if !LUMEN_SPLIT_GEOMETRY
    uint4 info;
#endif
};
// Geometry is the logical record; IntersectionPrimitive is its storage layout.
struct Geometry {
    float4 p0, p1, p2;
    uint4 info;
};
struct IntersectionPrimitive {
    float4 p0, p1, p2;
#if !LUMEN_DENSE_GEOMETRY
    uint4 info;
#endif
};
Geometry unpack_geometry(device const IntersectionPrimitive &g) {
#if LUMEN_DENSE_GEOMETRY
    uint encoded = as_type<uint>(g.p1.w);
    return {g.p0, g.p1, g.p2, uint4(encoded & 1, encoded >> 1, as_type<uint>(g.p2.w), 0)};
#else
    return {g.p0, g.p1, g.p2, g.info};
#endif
}
float2 vertex_uv0(device const Primitive &p) {
#if LUMEN_DENSE_ATTRIBUTES
    return float2(p.n0.w, p.n1.w);
#else
    return p.uv01.xy;
#endif
}
float2 vertex_uv1(device const Primitive &p) {
#if LUMEN_DENSE_ATTRIBUTES
    return float2(p.n2.w, p.uv.x);
#else
    return p.uv01.zw;
#endif
}
float2 vertex_uv2(device const Primitive &p) {
#if LUMEN_DENSE_ATTRIBUTES
    return p.uv.yz;
#else
    return p.uv2.xy;
#endif
}
#if LUMEN_WIDE_BVH
struct Node {
    float4 lower_x, lower_y, lower_z, upper_x, upper_y, upper_z;
    uint4 first, count, axes;
};
#else
struct Node {
    float4 lo, hi;
#if !LUMEN_COMPACT_NODES
    uint4 data;
#endif
};
uint4 node_data(device const Node &node) {
#if LUMEN_COMPACT_NODES
    uint first = as_type<uint>(node.lo.w), encoded = as_type<uint>(node.hi.w);
    uint tag = encoded >> 30, value = encoded & 0x3fffffff;
    return tag == 3 ? uint4(first, value, 0, 0) : uint4(first, 0, value, tag);
#else
    return node.data;
#endif
}
#endif
Geometry geometry_at(device const Primitive *p,
                                  device const IntersectionPrimitive *geometry, uint i) {
#if LUMEN_SPLIT_GEOMETRY
    return unpack_geometry(geometry[i]);
#else
    if (compact_geometry) return unpack_geometry(geometry[i]);
    return {p[i].p0, p[i].p1, p[i].p2, p[i].info};
#endif
}
struct Material {
    float4 base, emission, surface, absorption, detail;
    uint4 maps;
};
struct Light {
    uint primitive, pad;
    float area, cdf;
};
struct Camera {
    float4 origin, forward, right, up;
};
struct Medium {
    float4 lo, hi, scattering, parameters;
};
struct Params {
    Camera camera;
    float4 sky, horizon, environment;
    uint4 image, config;
    float4 adaptive;
    uint4 tile;
    Medium medium;
};
struct Ray {
    float3 o, d;
};
struct Hit {
    float t;
    float3 p, n, g;
    float2 uv;
    uint primitive, material;
    bool front;
};
struct SurfaceIntersection {
    float t;
    float2 barycentric;
};
constant float PI = 3.14159265358979323846f;
uint hash32(uint x) {
    x ^= x >> 16;
    x *= 0x7feb352du;
    x ^= x >> 15;
    x *= 0x846ca68bu;
    return x ^ (x >> 16);
}
float random_float(thread uint &r) {
    r ^= r << 13;
    r ^= r >> 17;
    r ^= r << 5;
    return float(r >> 8) * (1.0f / 16777216.0f);
}
float2 disk(thread uint &r) {
    float radius = sqrt(random_float(r));
    float angle = 2 * PI * random_float(r);
    return radius * float2(cos(angle), sin(angle));
}
float3 sphere_sample(thread uint &r) {
    float z = 1 - 2 * random_float(r);
    float a = 2 * PI * random_float(r);
    float v = sqrt(max(0.0f, 1 - z * z));
    return float3(v * cos(a), z, v * sin(a));
}
float max3(float3 v) { return max(v.x, max(v.y, v.z)); }
float lum(float3 v) { return dot(v, float3(.2126f, .7152f, .0722f)); }
float power(float a, float b) {
    a *= a;
    return a / max(a + b * b, 1e-30f);
}
float3 offset(float3 p, float3 n, float3 d) {
    return p + n * (dot(n, d) >= 0 ? 1.0f : -1.0f) * 2e-5f * max(1.0f, max3(abs(p)));
}
float3 tangent(float3 n) {
    return normalize(cross(abs(n.z) < .999f ? float3(0, 0, 1) : float3(1, 0, 0), n));
}
float3 to_world(float3 v, float3 n) {
    float3 t = tangent(n);
    return t * v.x + cross(n, t) * v.y + n * v.z;
}
bool box_hit(float3 lo, float3 hi, Ray r, float limit) {
    float a = 0, b = limit;
    for (uint k = 0; k < 3; k++) {
        if (abs(r.d[k]) < 1e-20f) {
            if (r.o[k] < lo[k] || r.o[k] > hi[k])
                return false;
            continue;
        }
        float t0 = (lo[k] - r.o[k]) / r.d[k], t1 = (hi[k] - r.o[k]) / r.d[k];
        a = max(a, min(t0, t1));
        b = min(b, max(t0, t1));
        if (b < a)
            return false;
    }
    return true;
}
// Cache the reciprocal per ray. Expand slab intervals conservatively to absorb
// reciprocal rounding; exact primitive tests still decide every surface hit.
bool box_hit_cached(float3 lo, float3 hi, float3 origin, float3 inverse,
                    bool3 parallel, float limit) {
    if (any(parallel & ((origin < lo) | (origin > hi)))) return false;
    float3 a = (lo - origin) * inverse, b = (hi - origin) * inverse;
    float3 near_t = select(min(a, b), float3(-INFINITY), parallel);
    float3 far_t = select(max(a, b), float3(INFINITY), parallel);
    near_t -= abs(near_t) * 1e-6f;
    far_t += abs(far_t) * 1e-6f;
    return min(limit, min(far_t.x, min(far_t.y, far_t.z))) >= max(0.0f, max3(near_t));
}
// Visibility needs distance only for opaque surfaces. Keep normals and UVs out of
// that path, particularly the transcendental functions used for sphere UVs.
template<typename PrimitiveData>
bool primitive_intersection(device const PrimitiveData &p, Ray ray, float limit,
                            thread SurfaceIntersection &intersection) {
    float t;
#if LUMEN_DENSE_GEOMETRY
    uint kind = as_type<uint>(p.p1.w) & 1;
#else
    uint kind = p.info.x;
#endif
    if (kind == 0) {
        float3 oc = ray.o - p.p0.xyz;
        float b = dot(oc, ray.d);
        float3 perpendicular = oc - ray.d * b;
        float disc = p.p0.w * p.p0.w - dot(perpendicular, perpendicular);
        if (disc < 0)
            return false;
        float d = sqrt(disc);
        t = -b - d;
        if (t <= 1e-5f)
            t = -b + d;
        if (t <= 1e-5f || t >= limit)
            return false;
        intersection.barycentric = 0;
    } else {
        float3 e1 = p.p1.xyz, e2 = p.p2.xyz, q = cross(ray.d, e2);
        float det = dot(e1, q);
        float3 determinant_terms = abs(e1 * q);
        float determinant_scale = determinant_terms.x + determinant_terms.y + determinant_terms.z;
        if (abs(det) <= 1e-7f * determinant_scale)
            return false;
        float inv = 1 / det;
        float3 s = ray.o - p.p0.xyz;
        float u = dot(s, q) * inv;
        if (u < 0 || u > 1)
            return false;
        q = cross(s, e1);
        float w = dot(ray.d, q) * inv;
        if (w < 0 || u + w > 1)
            return false;
        t = dot(e2, q) * inv;
        if (t <= 1e-5f || t >= limit)
            return false;
        intersection.barycentric = float2(u, w);
    }
    intersection.t = t;
    return true;
}
Hit surface_hit(device const Primitive &p, Geometry g, Ray ray, uint index,
                SurfaceIntersection intersection) {
    Hit hit;
    float t = intersection.t;
    float3 outward, shading;
    if (g.info.x == 0) {
        float3 oc = ray.o - g.p0.xyz;
        outward = normalize(oc + ray.d * t);
        shading = outward;
        hit.uv = float2(.5f + atan2(outward.z, outward.x) / (2 * PI), .5f - asin(outward.y) / PI);
    } else {
        float u = intersection.barycentric.x, w = intersection.barycentric.y;
        hit.uv = vertex_uv0(p) * (1 - u - w) + vertex_uv1(p) * u + vertex_uv2(p) * w;
        outward = normalize(cross(g.p1.xyz, g.p2.xyz));
        shading = p.n0.xyz * (1 - u - w) + p.n1.xyz * u + p.n2.xyz * w;
        shading = dot(shading, shading) > 1e-12f ? normalize(shading) : outward;
    }
    if (dot(shading, outward) < 0)
        shading = -shading;
    hit.front = dot(ray.d, outward) < 0;
    hit.g = hit.front ? outward : -outward;
    hit.n = hit.front ? shading : -shading;
    if (dot(hit.n, -ray.d) <= 0)
        hit.n = hit.g;
    hit.t = t;
    hit.p = ray.o + t * ray.d;
    hit.primitive = index;
    hit.material = g.info.y;
    return hit;
}
float3 texture_sample(device const float4 *texels, device const uint4 *textures, uint index,
                      float2 uv);
float2 surface_uv(Material material, Hit hit);
float fresnel(float c, float ei, float et);
float thin_transmittance(float cosine, float ior) {
    float r = fresnel(clamp(abs(cosine), 0.0f, 1.0f), 1.0f, ior);
    return (1 - r) / (1 + r);
}

#if LUMEN_NATIVE_TRAVERSAL
struct NativePayload {
    device const Primitive *attributes;
    device const IntersectionPrimitive *geometry;
    device const Material *materials;
    device const float4 *texels;
    device const uint4 *textures;
};
struct NativeIntersection {
    bool accept [[accept_intersection]];
    float distance [[distance]];
};
// Native traversal changes only candidate selection. Keep the same robust sphere
// and triangle equations, opacity threshold, normals and texture coordinates.
[[intersection(bounding_box)]]
NativeIntersection native_intersection(float3 origin [[origin]], float3 direction [[direction]],
    uint index [[primitive_id]], float limit [[max_distance]], ray_data NativePayload &payload [[payload]]) {
    Ray ray = {origin, direction};
    SurfaceIntersection intersection;
    if (!primitive_intersection(payload.geometry[index], ray, limit, intersection)) return {false, 0};
    Material material = payload.materials[unpack_geometry(payload.geometry[index]).info.y];
    if (has_opacity && material.maps.w > 0) {
        Hit hit = surface_hit(payload.attributes[index], unpack_geometry(payload.geometry[index]), ray, index, intersection);
        if (texture_sample(payload.texels, payload.textures, material.maps.w - 1, surface_uv(material, hit)).x < .5f)
            return {false, 0};
    }
    return {true, intersection.t};
}
bool intersect_native(NativeContext native, device const Primitive *p,
    device const IntersectionPrimitive *geometry, Ray ray, float limit, thread Hit &hit,
    device const Material *materials, device const float4 *texels, device const uint4 *textures) {
    NativePayload payload = {p, geometry, materials, texels, textures};
    intersector<> query;
    query.accept_any_intersection(false);
    auto result = query.intersect(metal::raytracing::ray(ray.o, ray.d, 0, limit),
                                  native.acceleration, native.functions, payload);
    if (result.type == intersection_type::none) return false;
    SurfaceIntersection intersection;
    // Reconstruct using the original limit, so strict closest-hit comparisons
    // do not reject the distance already accepted by the intersection function.
    if (!primitive_intersection(geometry[result.primitive_id], ray, limit, intersection)) return false;
    hit = surface_hit(p[result.primitive_id], unpack_geometry(geometry[result.primitive_id]), ray, result.primitive_id, intersection);
    return true;
}
#endif

bool intersect_impl(device const Primitive *p, device const IntersectionPrimitive *geometry, device const Node *nodes, Ray ray, float limit,
               bool any_hit, thread Hit &hit, device const Material *materials,
               device const float4 *texels, device const uint4 *textures,
               bool transmit_sheets, thread float &visibility) {
    uint stack[LUMEN_BVH_STACK_SIZE];
    uint sp = 1;
    stack[0] = LUMEN_WIDE_BVH ? 0xffffffffu : 0;
    bool found = false;
    uint closest_primitive = 0;
    SurfaceIntersection closest_intersection;
#if LUMEN_CACHED_BOUNDS || LUMEN_WIDE_BVH
    bool3 parallel = abs(ray.d) < float3(1e-20f);
    float3 inverse = 1.0f / select(ray.d, float3(1), parallel);
#endif
    while (sp > 0) {
#if LUMEN_WIDE_BVH
        uint entry = stack[--sp], address = 0;
        uint4 data = 0;
        if (entry != 0xffffffffu) {
            device const Node &parent = nodes[entry >> 2];
            uint lane = entry & 3;
            data = uint4(parent.first[lane], parent.count[lane], 0, 0);
            address = data.x;
        }
        if (data.y == 0) {
            device const Node &node = nodes[address];
            float4 near_t = 0, far_t = limit;
            bool4 active = node.count != uint4(0xffffffffu);
            for (uint axis = 0; axis < 3; axis++) {
                float4 lo = axis == 0 ? node.lower_x : (axis == 1 ? node.lower_y : node.lower_z);
                float4 hi = axis == 0 ? node.upper_x : (axis == 1 ? node.upper_y : node.upper_z);
                if (parallel[axis]) {
                    active &= (ray.o[axis] >= lo) & (ray.o[axis] <= hi);
                } else {
                    float4 a = (lo - ray.o[axis]) * inverse[axis], b = (hi - ray.o[axis]) * inverse[axis];
                    float4 n = min(a, b), f = max(a, b);
                    near_t = max(near_t, n - abs(n) * 1e-6f);
                    far_t = min(far_t, f + abs(f) * 1e-6f);
                }
            }
            active &= far_t >= near_t;
            // Preserve the original binary visit order, including coincident surfaces.
            uint far_branch = ray.d[node.axes.x] >= 0 ? 1 : 0;
            for (uint visit = 0; visit < 2; visit++) {
                uint branch = far_branch ^ visit;
                uint far_child = ray.d[node.axes[branch + 1]] >= 0 ? 1 : 0;
                for (uint child = 0; child < 2; child++) {
                    uint lane = branch * 2 + (far_child ^ child);
                    if (active[lane]) stack[sp++] = address * 4 + lane;
                }
            }
            continue;
        }
#else
        device const Node &node = nodes[stack[--sp]];
#if LUMEN_CACHED_BOUNDS
        if (!box_hit_cached(node.lo.xyz, node.hi.xyz, ray.o, inverse, parallel, limit)) continue;
#else
        if (!box_hit(node.lo.xyz, node.hi.xyz, ray, limit)) continue;
#endif
        uint4 data = node_data(node);
#endif
        if (data.y > 0) {
            for (uint i = data.x; i < data.x + data.y; i++) {
                SurfaceIntersection intersection;
#if LUMEN_SPLIT_GEOMETRY
                bool found_intersection = primitive_intersection(geometry[i], ray, limit, intersection);
#else
                bool found_intersection = compact_geometry
                    ? primitive_intersection(geometry[i], ray, limit, intersection)
                    : primitive_intersection(p[i], ray, limit, intersection);
#endif
                if (found_intersection) {
#if LUMEN_VISIBILITY_FLAGS
                    // Immutable classification removes material-buffer reads for
                    // ordinary blockers. Cutouts and thin sheets retain full evaluation.
                    uint flags = geometry[i].info.w;
                    bool needs_material = (has_opacity && (flags & 1) != 0) ||
                                          (transmit_sheets && (flags & 2) != 0);
                    if (any_hit && !needs_material) return true;
                    if (needs_material) {
#endif
                    uint material_index = geometry_at(p, geometry, i).info.y;
                    Material material = materials[material_index];
                    if (any_hit && (!has_opacity || material.maps.w == 0) &&
                        (!transmit_sheets || material.surface.z != 2))
                        return true;
                    if ((has_opacity && material.maps.w > 0) || (transmit_sheets && material.surface.z == 2)) {
                        Hit candidate = surface_hit(p[i], geometry_at(p, geometry, i), ray, i, intersection);
                        if ((has_opacity && material.maps.w > 0) && texture_sample(texels, textures, material.maps.w - 1,
                                                                  surface_uv(material, candidate))
                                                           .x < .5f)
                            continue;
                        if (transmit_sheets && material.surface.z == 2) {
                            visibility *= thin_transmittance(dot(candidate.g, ray.d), material.surface.w);
                            continue;
                        }
                    }
#if LUMEN_VISIBILITY_FLAGS
                    }
#endif
                    if (any_hit)
                        return true;
                    found = true;
                    limit = intersection.t;
                    closest_primitive = i;
                    closest_intersection = intersection;
                }
            }
        }
#if !LUMEN_WIDE_BVH
        else {
            bool positive = ray.d[data.w] >= 0;
            stack[sp++] = positive ? data.z : data.x;
            stack[sp++] = positive ? data.x : data.z;
        }
#endif
    }
    // Only the nearest accepted surface needs shading attributes. Masked surfaces
    // and transmitting sheets above still evaluate them during traversal.
    if (found)
        hit = surface_hit(p[closest_primitive], geometry_at(p, geometry, closest_primitive), ray, closest_primitive, closest_intersection);
    return found;
}
bool intersect(device const Primitive *p, device const IntersectionPrimitive *geometry, device const Node *nodes, Ray ray, float limit,
               bool any_hit, thread Hit &hit, device const Material *materials,
               device const float4 *texels, device const uint4 *textures NATIVE_PARAMETER) {
#if LUMEN_NATIVE_TRAVERSAL
    if (!any_hit) return intersect_native(native, p, geometry, ray, limit, hit, materials, texels, textures);
#endif
    float visibility = 1;
    return intersect_impl(p, geometry, nodes, ray, limit, any_hit, hit, materials, texels, textures, false, visibility);
}
float shadow_transmittance(device const Primitive *p, device const IntersectionPrimitive *geometry, device const Node *nodes, Ray ray, float limit,
                           device const Material *materials, device const float4 *texels, device const uint4 *textures) {
    float visibility = 1;
    Hit blocker;
    bool blocked = intersect_impl(p, geometry, nodes, ray, limit, true, blocker, materials, texels, textures, true, visibility);
    return blocked ? 0 : visibility;
}
float3 base_color(Material m, float3 p) {
    float3 q = p * m.detail.x;
    float k = 1;
    switch (uint(m.detail.y)) {
    case 1: {
        float t = abs(sin(q.x * 1.7f + q.z * 2.2f + 2 * sin(q.z * .7f + q.x * .5f) +
                          .3f * sin(q.x * 9 + q.z * 7)));
        k = .58f + .42f * pow(t, .18f);
        break;
    }
    case 2:
        k = .68f + .25f * sin(q.x * 3 + .7f * sin(q.z * .8f) + .25f * sin(q.z * 3)) +
            .07f * sin(q.x * 47 + q.z);
        break;
    case 3:
        k = (int(floor(q.x) + floor(q.z)) & 1) == 0 ? 1 : .35f;
        break;
    }
    return m.base.xyz * k;
}
float smith(float n, float a2) { return 2 * n / max(n + sqrt(a2 + (1 - a2) * n * n), 1e-10f); }
float3 eval(Material m, float3 color, float3 n, float3 wo, float3 wi, thread float &pdf) {
    float nv = max(dot(n, wo), 0.0f), nl = dot(n, wi);
    pdf = 0;
    if (nv <= 0 || nl == 0)
        return float3(0);
    if (nl < 0) {
        pdf = -nl * m.absorption.w / PI;
        return color * (m.absorption.w / PI);
    }
    float3 hv = wi + wo;
    if (dot(hv, hv) < 1e-20f)
        return float3(0);
    float3 h = normalize(hv);
    float nh = max(dot(n, h), 0.0f), vh = max(dot(wo, h), 0.0f);
    float alpha = max(m.surface.x, .025f);
    alpha *= alpha;
    float a2 = alpha * alpha;
    float q = nh * nh * (a2 - 1) + 1;
    float d = a2 / PI / max(q * q, 1e-16f), g1 = smith(nv, a2), g2 = smith(nl, a2);
    float3 f0 = mix(float3(.04f), color, m.surface.y), f = f0 + (1 - f0) * pow(1 - vh, 5.0f);
    float spec_p = .25f + .5f * m.surface.y;
    pdf = (spec_p * d * g1 / max(4 * nv, 1e-12f) + (1 - spec_p) * nl / PI) * (1 - m.absorption.w);
    return ((1 - f) * color * ((1 - m.surface.y) / PI) +
           f * (d * g1 * g2 / max(4 * nv * nl, 1e-12f))) * (1 - m.absorption.w);
}
float3 bsdf_sample(Material m, float3 n, float3 wo, thread uint &rng) {
    if (m.absorption.w > 0 && random_float(rng) < m.absorption.w) {
        float2 d = disk(rng);
        return -to_world(float3(d, sqrt(max(0.0f, 1 - dot(d, d)))), n);
    }
    if (random_float(rng) >= .25f + .5f * m.surface.y) {
        float2 d = disk(rng);
        return to_world(float3(d, sqrt(max(0.0f, 1 - dot(d, d)))), n);
    }
    float3 t = tangent(n), b = cross(n, t);
    float alpha = max(m.surface.x, .025f);
    alpha *= alpha;
    float3 vh = normalize(float3(alpha * dot(wo, t), alpha * dot(wo, b), dot(wo, n)));
    float l = vh.x * vh.x + vh.y * vh.y;
    float3 t1 = l > 0 ? float3(-vh.y, vh.x, 0) / sqrt(l) : float3(1, 0, 0), t2 = cross(vh, t1);
    float2 xy = disk(rng);
    float s = .5f * (1 + vh.z);
    xy.y = (1 - s) * sqrt(max(0.0f, 1 - xy.x * xy.x)) + s * xy.y;
    float3 nh = xy.x * t1 + xy.y * t2 + sqrt(max(0.0f, 1 - dot(xy, xy))) * vh;
    float3 h = to_world(normalize(float3(alpha * nh.x, alpha * nh.y, max(nh.z, 0.0f))), n);
    return normalize(-wo + 2 * dot(wo, h) * h);
}
float fresnel(float c, float ei, float et) {
    float st = ei / et * sqrt(max(0.0f, 1 - c * c));
    if (st >= 1)
        return 1;
    float ct = sqrt(1 - st * st);
    float rs = (ei * c - et * ct) / (ei * c + et * ct),
          rp = (et * c - ei * ct) / (et * c + ei * ct);
    return .5f * (rs * rs + rp * rp);
}
float light_prob(device const Light *lights, uint i) {
    return lights[i].cdf - (i == 0 ? 0 : lights[i - 1].cdf);
}
float3 texture_texel(device const float4 *texels, uint4 info, int x, int y) {
    x = ((x % int(info.y)) + int(info.y)) % int(info.y);
    y = ((y % int(info.z)) + int(info.z)) % int(info.z);
    return texels[info.x + uint(y) * info.y + uint(x)].xyz;
}
float3 texture_sample(device const float4 *texels, device const uint4 *textures, uint index,
                      float2 uv) {
    uint4 info = textures[index];
    float x = fract(uv.x) * float(info.y) - .5f;
    float y = (1 - fract(uv.y)) * float(info.z) - .5f;
    int ix = int(floor(x)), iy = int(floor(y));
    float3 a =
        mix(texture_texel(texels, info, ix, iy), texture_texel(texels, info, ix + 1, iy), fract(x));
    float3 b = mix(texture_texel(texels, info, ix, iy + 1),
                   texture_texel(texels, info, ix + 1, iy + 1), fract(x));
    return mix(a, b, fract(y));
}
float3 environment_radiance(float3 direction, constant Params &params,
                            device const float4 *texels, device const uint4 *textures) {
    if (params.environment.x > 0) {
        uint index = uint(params.environment.x) - 1;
        float u = atan2(direction.z, direction.x) / (2 * PI) + .5f + params.environment.z / (2 * PI);
        float v = 1 - acos(clamp(direction.y, -1.0f, 1.0f)) / PI;
        float half_texel = .5f / float(textures[index].z);
        float3 radiance = texture_sample(texels, textures, index, float2(u, clamp(v, half_texel, 1 - half_texel))) * params.environment.y;
        float t = params.environment.w > 0 ? clamp(direction.y / params.environment.w, 0.0f, 1.0f) : 1.0f;
        return mix(params.horizon.xyz, radiance, t * t * (3 - 2 * t));
    }
    return mix(params.horizon.xyz, params.sky.xyz, clamp(direction.y * .5f + .5f, 0.0f, 1.0f));
}
float2 surface_uv(Material material, Hit hit) {
    if (material.detail.w >= 0)
        return hit.uv * material.detail.w;
    float3 n = abs(hit.n);
    float2 uv = n.y >= n.x && n.y >= n.z ? hit.p.xz : (n.x > n.z ? hit.p.zy : hit.p.xy);
    return uv * -material.detail.w;
}
void surface_shading(thread Material &material, thread Hit &hit, float3 outgoing,
                     device const Primitive *primitives, device const IntersectionPrimitive *geometry,
                     device const float4 *texels, device const uint4 *textures) {
    if (material.surface.z > .5f || all(material.maps == uint4(0)))
        return;
    float2 uv = surface_uv(material, hit);
    if (material.maps.y > 0)
        material.surface.x *=
            clamp(texture_sample(texels, textures, material.maps.y - 1, uv).x, 0.0f, 1.0f);
    if (material.maps.z > 0)
        material.surface.y *=
            clamp(texture_sample(texels, textures, material.maps.z - 1, uv).x, 0.0f, 1.0f);
    if (material.maps.x == 0)
        return;
    float3 mapped = texture_sample(texels, textures, material.maps.x - 1, uv) * 2 - 1;
    device const Primitive &p = primitives[hit.primitive];
    Geometry g = geometry_at(primitives, geometry, hit.primitive);
    float3 t, b;
    if (material.detail.w < 0) {
        float3 n = abs(hit.n);
        if (n.y >= n.x && n.y >= n.z) {
            t = float3(1, 0, 0);
            b = float3(0, 0, 1);
        } else if (n.x > n.z) {
            t = float3(0, 0, 1);
            b = float3(0, 1, 0);
        } else {
            t = float3(1, 0, 0);
            b = float3(0, 1, 0);
        }
    } else if (g.info.x == 1) {
        float2 du = vertex_uv1(p) - vertex_uv0(p), dv = vertex_uv2(p) - vertex_uv0(p);
        float determinant = du.x * dv.y - du.y * dv.x;
        if (abs(determinant) <= 1e-10f)
            return;
        t = (g.p1.xyz * dv.y - g.p2.xyz * du.y) / determinant;
        b = (g.p2.xyz * du.x - g.p1.xyz * dv.x) / determinant;
    } else
        return;
    t -= hit.n * dot(hit.n, t);
    if (dot(t, t) < 1e-20f)
        return;
    t = normalize(t);
    b = cross(hit.n, t) * (dot(cross(hit.n, t), b) < 0 ? -1.0f : 1.0f);
    float3 candidate = normalize(t * mapped.x + b * mapped.y + hit.n * max(mapped.z, .01f));
    if (dot(candidate, hit.g) > .05f && dot(candidate, outgoing) > .05f)
        hit.n = candidate;
}
float3 surface_color(Material material, Hit hit, device const float4 *texels,
                     device const uint4 *textures) {
    float3 color = base_color(material, hit.p);
    if (material.detail.z < 1)
        return color;
    float2 uv = hit.uv * material.detail.w;
    if (material.detail.w < 0) {
        float3 n = abs(hit.n);
        uv = n.y >= n.x && n.y >= n.z ? hit.p.xz : (n.x > n.z ? hit.p.zy : hit.p.xy);
        uv *= -material.detail.w;
    }
    return color * mix(float3(1), texture_sample(texels, textures, uint(material.detail.z) - 1, uv),
                       material.base.w);
}
bool medium_interval(Medium medium, Ray ray, float limit, thread float &near, thread float &far) {
    near = 0;
    far = limit;
    for (uint axis = 0; axis < 3; axis++) {
        if (abs(ray.d[axis]) < 1e-20f) {
            if (ray.o[axis] < medium.lo[axis] || ray.o[axis] > medium.hi[axis])
                return false;
        } else {
            float a = (medium.lo[axis] - ray.o[axis]) / ray.d[axis];
            float b = (medium.hi[axis] - ray.o[axis]) / ray.d[axis];
            near = max(near, min(a, b));
            far = min(far, max(a, b));
        }
    }
    return far > near;
}
float noise_lattice(int3 cell) {
    return float(hash32(uint(cell.x) * 73856093u ^ uint(cell.y) * 19349663u ^
                        uint(cell.z) * 83492791u) >>
                 8) /
           16777216.0f;
}
float value_noise3(float3 position) {
    int3 cell = int3(floor(position));
    float3 f = fract(position);
    float3 t = f * f * (3 - 2 * f);
    float layers[2];
    for (int z = 0; z < 2; z++) {
        float rows[2];
        for (int y = 0; y < 2; y++)
            rows[y] =
                mix(noise_lattice(cell + int3(0, y, z)), noise_lattice(cell + int3(1, y, z)), t.x);
        layers[z] = mix(rows[0], rows[1], t.y);
    }
    return mix(layers[0], layers[1], t.z);
}
float incense_density(Medium medium, float3 position) {
    float3 source = float3(medium.parameters.x, medium.lo.y, medium.parameters.y);
    float3 local = (position - source) / medium.parameters.z;
    float t = local.y;
    if (t < 0 || t >= 1 || abs(local.x) >= .28f || abs(local.z) >= .28f)
        return 0;
    float curl = smoothstep(.10f, .60f, t);
    float x = local.x - (.10f * t + .07f * curl * sin(11 * t));
    float z = local.z - (.035f * t + .045f * curl * sin(14 * t + .6f));
    float angle = 10 * t + 1.4f * sin(7 * t);
    float sine = sin(angle), cosine = cos(angle);
    float radius = .0032f + .010f * t + .036f * t * t;
    float u = (cosine * x + sine * z) / radius;
    float v = (-sine * x + cosine * z) / radius;
    float core = exp(-2 * (u * u / .36f + v * v));
    float secondary = .35f * curl * exp(-2 * ((u - .85f) * (u - .85f) / .25f + v * v));
    float modulation = .75f + .25f * value_noise3(local * float3(45, 18, 45));
    float fade = 1 - smoothstep(.65f, 1.0f, t);
    return clamp((core + secondary) * modulation * fade * .85f / (1 + 3 * t), 0.0f, 1.0f);
}
float medium_density(Medium medium, float3 position) {
    if (medium.scattering.w == 0)
        return 1;
    if (medium.scattering.w == 2)
        return incense_density(medium, position);
    float3 center = (medium.lo.xyz + medium.hi.xyz) * .5f;
    float3 half_size = (medium.hi.xyz - medium.lo.xyz) * .5f;
    float3 q = (position - center) / half_size;
    float shape = length((q - float3(0, -.2f, 0)) / float3(.65f, .5f, .65f)) - 1;
    shape = min(shape, length((q - float3(-.45f, -.15f, .05f)) / float3(.42f, .5f, .5f)) - 1);
    shape = min(shape, length((q - float3(-.12f, .22f, -.05f)) / float3(.4f, .68f, .45f)) - 1);
    shape = min(shape, length((q - float3(.42f, 0, 0)) / float3(.44f, .6f, .48f)) - 1);
    float3 p = position * medium.parameters.x;
    float noise =
        .55f * value_noise3(p) + .3f * value_noise3(p * 2.03f) + .15f * value_noise3(p * 4.07f);
    float density = clamp((-shape + .7f * (noise - .5f)) * 7, 0.0f, 1.0f);
    return density * density * (3 - 2 * density);
}
float medium_sample(Medium medium, Ray ray, float limit, thread uint &rng) {
    if (!has_volumes || medium.lo.w <= 0)
        return -1;
    float distance, end;
    if (!medium_interval(medium, ray, limit, distance, end))
        return -1;
    while (true) {
        distance += -log(1 - random_float(rng)) / medium.lo.w;
        if (distance >= end)
            return -1;
        if (medium.scattering.w == 0 ||
            random_float(rng) < medium_density(medium, ray.o + distance * ray.d))
            return distance;
    }
}
float medium_transmittance(Medium medium, Ray ray, float limit, thread uint &rng) {
    if (!has_volumes || medium.lo.w <= 0)
        return 1;
    float distance, end;
    if (!medium_interval(medium, ray, limit, distance, end))
        return 1;
    if (medium.scattering.w == 0)
        return exp(-medium.lo.w * (end - distance));
    float weight = 1;
    while (true) {
        distance += -log(1 - random_float(rng)) / medium.lo.w;
        if (distance >= end)
            return weight;
        weight *= 1 - medium_density(medium, ray.o + distance * ray.d);
        if (weight == 0)
            return 0;
    }
}
float medium_phase(Medium medium, float cosine) {
    float g = medium.hi.w;
    return (1 - g * g) / (4 * PI * pow(max(1e-8f, 1 + g * g - 2 * g * cosine), 1.5f));
}
float3 sample_phase(Medium medium, float3 forward, thread uint &rng) {
    float g = medium.hi.w, u = random_float(rng);
    float c;
    if (abs(g) < 1e-3f)
        c = 1 - 2 * u;
    else {
        float ratio = (1 - g * g) / (1 - g + 2 * g * u);
        c = (1 + g * g - ratio * ratio) / (2 * g);
    }
    c = clamp(c, -1.0f, 1.0f);
    float angle = 2 * PI * random_float(rng), s = sqrt(max(0.0f, 1 - c * c));
    return to_world(float3(s * cos(angle), s * sin(angle), c), forward);
}
float3 volume_light(device const Primitive *primitives, device const IntersectionPrimitive *geometry, device const Node *nodes,
                    device const Material *materials, device const Light *lights,
                    constant Params &params, float3 position, float3 forward, thread uint &rng,
                    device const float4 *texels, device const uint4 *textures) {
    if (params.config.z == 0)
        return float3(0);
    float choice = random_float(rng);
    uint lo = 0, hi = params.config.z;
    while (lo < hi) {
        uint mid = (lo + hi) / 2;
        if (lights[mid].cdf < choice)
            lo = mid + 1;
        else
            hi = mid;
    }
    uint index = min(lo, params.config.z - 1);
    device const Light &light = lights[index];
    Geometry primitive = geometry_at(primitives, geometry, light.primitive);
    float3 light_position, normal;
    if (primitive.info.x == 0) {
        normal = sphere_sample(rng);
        light_position = primitive.p0.xyz + normal * primitive.p0.w;
    } else {
        float u = sqrt(random_float(rng)), v = random_float(rng);
        light_position =
            primitive.p0.xyz + primitive.p1.xyz * (u * (1 - v)) + primitive.p2.xyz * (u * v);
        normal = normalize(cross(primitive.p1.xyz, primitive.p2.xyz));
    }
    float3 difference = light_position - position;
    float distance_squared = dot(difference, difference);
    float3 direction = normalize(difference);
    float cosine = abs(dot(normal, -direction));
    if (cosine < 1e-7f)
        return float3(0);
    float pdf = light_prob(lights, index) * distance_squared / (cosine * light.area);
    float phase = medium_phase(params.medium, dot(forward, direction));
    float3 target = offset(light_position, normal, -direction), shadow = target - position;
    Ray ray = {position, normalize(shadow)};
    float visibility = shadow_transmittance(primitives, geometry, nodes, ray, length(shadow) - 1e-5f,
                                           materials, texels, textures);
    if (visibility <= 0)
        return float3(0);
    float transmittance = medium_transmittance(params.medium, ray, length(shadow), rng);
    return materials[primitive.info.y].emission.xyz *
           (transmittance * visibility * phase * power(pdf, phase * visibility) / max(pdf, 1e-20f));
}
float3 trace(device const Primitive *primitives, device const IntersectionPrimitive *geometry, device const Node *nodes,
             device const Material *mats, device const Light *lights, constant Params &params,
             Ray ray, thread uint &rng, device const float4 *texels, device const uint4 *textures NATIVE_PARAMETER) {
    float3 radiance = 0, beta = 1, prev_p = ray.o;
    float prev_pdf = 0, eta_scale = 1;
    bool delta = true;
    uint media[16];
    uint medium_count = 0;
    for (uint depth = 0; depth < params.config.x; depth++) {
        Hit h;
        bool surface_found =
            intersect(primitives, geometry, nodes, ray, INFINITY, false, h, mats, texels, textures NATIVE_ARGUMENT);
        float collision = medium_count == 0 ? medium_sample(params.medium, ray,
                                                            surface_found ? h.t : INFINITY, rng)
                                            : -1;
        if (collision >= 0) {
            float3 position = ray.o + collision * ray.d;
            beta *= params.medium.scattering.xyz;
            radiance += beta * volume_light(primitives, geometry, nodes, mats, lights, params, position,
                                            ray.d, rng, texels, textures);
            float3 direction = sample_phase(params.medium, ray.d, rng);
            prev_pdf = medium_phase(params.medium, dot(ray.d, direction));
            prev_p = position;
            delta = false;
            ray = {position, direction};
            if (depth >= 3) {
                float survival = clamp(max3(beta), .05f, .95f);
                if (random_float(rng) > survival)
                    break;
                beta /= survival;
            }
            continue;
        }
        if (!surface_found) {
            radiance += beta * environment_radiance(ray.d, params, texels, textures);
            break;
        }
        if (medium_count > 0)
            beta *= exp(-mats[media[medium_count - 1]].absorption.xyz * h.t);
        Material m = mats[h.material];
        Hit shading_hit = h;
        surface_shading(m, shading_hit, -ray.d, primitives, geometry, texels, textures);
        if (max3(m.emission.xyz) > 0) {
            float weight = 1;
            uint lid = geometry_at(primitives, geometry, h.primitive).info.z;
            if (!delta && lid > 0) {
                uint i = lid - 1;
                float c = abs(dot(h.g, -ray.d));
                float pdf = light_prob(lights, i) * dot(h.p - prev_p, h.p - prev_p) /
                            max(lights[i].area * c, 1e-12f);
                weight = power(prev_pdf, pdf);
            }
            radiance += beta * m.emission.xyz * weight;
            break;
        }
        float3 n = shading_hit.n, wo = -ray.d;
        if (m.surface.z == 2) {
            float c = clamp(dot(h.g, wo), 0.0f, 1.0f);
            float transmission = thin_transmittance(c, m.surface.w);
            float3 direction = ray.d;
            if (random_float(rng) < 1 - transmission) {
                direction += 2 * c * h.g;
                delta = true;
            } else {
                prev_pdf *= transmission;
            }
            ray = {offset(h.p, h.g, direction), direction};
        } else if (m.surface.z > .5f) {
            float ei = medium_count > 0 ? mats[media[medium_count - 1]].surface.w : 1;
            float et = h.front ? m.surface.w
                               : (medium_count > 1 ? mats[media[medium_count - 2]].surface.w : 1);
            float c = clamp(dot(h.g, wo), 0.0f, 1.0f), f = fresnel(c, ei, et);
            float3 direction;
            if (random_float(rng) < f)
                direction = ray.d + 2 * c * h.g;
            else {
                float eta = ei / et;
                float3 perp = eta * (ray.d + c * h.g);
                direction = perp - sqrt(max(0.0f, 1 - dot(perp, perp))) * h.g;
                beta *= eta * eta;
                eta_scale /= eta * eta;
                if (h.front) {
                    if (medium_count >= 16)
                        break;
                    media[medium_count++] = h.material;
                } else {
                    medium_count = medium_count > 0 ? medium_count - 1 : 0;
                }
            }
            ray = {offset(h.p, h.g, direction), normalize(direction)};
            delta = true;
        } else {
            float3 color = surface_color(m, h, texels, textures);
            if (params.config.z > 0) {
                float pick = random_float(rng);
                uint lo = 0, hi = params.config.z;
                while (lo < hi) {
                    uint mid = (lo + hi) / 2;
                    if (lights[mid].cdf < pick)
                        lo = mid + 1;
                    else
                        hi = mid;
                }
                uint li = min(lo, params.config.z - 1);
                device const Light &light = lights[li];
                Geometry p = geometry_at(primitives, geometry, light.primitive);
                float3 lp, ln;
                if (p.info.x == 0) {
                    ln = sphere_sample(rng);
                    lp = p.p0.xyz + ln * p.p0.w;
                } else {
                    float u = sqrt(random_float(rng)), w = random_float(rng);
                    lp = p.p0.xyz + p.p1.xyz * (u * (1 - w)) + p.p2.xyz * (u * w);
                    ln = normalize(cross(p.p1.xyz, p.p2.xyz));
                }
                float3 diff = lp - h.p;
                float dist2 = dot(diff, diff), dist = sqrt(dist2);
                float3 wi = diff / dist;
                float cl = abs(dot(ln, -wi)), c = abs(dot(n, wi));
                if (c > 0 && cl > 1e-7f && dot(n, wi) * dot(h.g, wi) > 0 &&
                    (dot(h.g, wi) > 0 || m.absorption.w > 0)) {
                    float pdf = light_prob(lights, li) * dist2 / (cl * light.area), bsdf_pdf;
                    float3 f = eval(m, color, n, wo, wi, bsdf_pdf);
                    float3 origin = offset(h.p, h.g, wi), end = offset(lp, ln, -wi),
                           shadow = end - origin;
                    Ray sr = {origin, normalize(shadow)};
                    float visibility = shadow_transmittance(primitives, geometry, nodes, sr, length(shadow) - 1e-5f,
                                                            mats, texels, textures);
                    if (visibility > 0)
                        radiance += beta *
                                    medium_transmittance(params.medium, sr, length(shadow), rng) *
                                    visibility *
                                    f * mats[p.info.y].emission.xyz *
                                    (c * power(pdf, bsdf_pdf * visibility) / max(pdf, 1e-20f));
                }
            }
            float3 wi = bsdf_sample(m, n, wo, rng);
            if (dot(n, wi) * dot(h.g, wi) <= 0 || (dot(wi, h.g) <= 0 && m.absorption.w <= 0))
                break;
            float pdf;
            float3 f = eval(m, color, n, wo, wi, pdf);
            if (pdf <= 1e-12f)
                break;
            beta *= f * (abs(dot(n, wi)) / pdf);
            prev_pdf = pdf;
            prev_p = h.p;
            delta = false;
            ray = {offset(h.p, h.g, wi), wi};
        }
        if (!all(isfinite(beta)) || max3(beta) <= 0)
            break;
        if (depth >= 3) {
            float survival = clamp(max3(beta) * eta_scale, .05f, .95f);
            if (random_float(rng) > survival)
                break;
            beta /= survival;
        }
    }
    return radiance;
}
uint compact_even_bits(uint value) {
    value &= 0x5555u;
    value = (value | (value >> 1)) & 0x3333u;
    value = (value | (value >> 2)) & 0x0f0fu;
    return (value | (value >> 4)) & 0x00ffu;
}
uint2 tile_coordinates(uint local_id, uint4 tile) {
#if LUMEN_MORTON_ORDER
    if (tile.z == 128 && tile.w == 128)
        return tile.xy + uint2(compact_even_bits(local_id), compact_even_bits(local_id >> 1));
#endif
    // Partial edge tiles retain their dense row-major coverage.
    return tile.xy + uint2(local_id % tile.z, local_id / tile.z);
}

kernel void
pathtrace(device const Primitive *primitives [[buffer(0)]], device const Node *nodes [[buffer(1)]],
          device const Material *materials [[buffer(2)]], device const Light *lights [[buffer(3)]],
          device float4 *film [[buffer(4)]], device float4 *moments [[buffer(5)]],
          constant Params &params [[buffer(6)]], device const float4 *texels [[buffer(7)]],
          device const uint4 *textures [[buffer(8)]],
          device const IntersectionPrimitive *geometry [[buffer(9)]],
          NATIVE_BINDINGS uint local_id [[thread_position_in_grid]]) {
    NATIVE_CONTEXT
    if (local_id >= params.tile.z * params.tile.w)
        return;
    uint2 pixel = tile_coordinates(local_id, params.tile);
    uint id = pixel.y * params.image.x + pixel.x;
    if (id >= params.image.x * params.image.y)
        return;
    float4 sum = film[id], moment = moments[id];
    if (moment.z > 0)
        return;
    uint x = id % params.image.x, y = id / params.image.x;
    for (uint sample = params.image.z; sample < params.image.w; sample++) {
        uint rng = hash32(id ^ hash32(sample + params.config.y * 747796405u)) | 1u;
        float sx = 2 * (float(x) + random_float(rng)) / float(params.image.x) - 1,
              sy = 1 - 2 * (float(y) + random_float(rng)) / float(params.image.y);
        Camera c = params.camera;
        float3 d = c.forward.xyz + sx * c.right.w * c.right.xyz + sy * c.up.w * c.up.xyz;
        float2 xy = disk(rng);
        float3 o = (c.right.xyz * xy.x + c.up.xyz * xy.y) * c.origin.w;
        Ray ray = {c.origin.xyz + o, normalize(d * c.forward.w - o)};
        float3 color =
            trace(primitives, geometry, nodes, materials, lights, params, ray, rng, texels, textures NATIVE_ARGUMENT);
        if (!all(isfinite(color))) {
            moment.w += 1;
            color = 0;
        }
        sum.xyz += color;
        sum.w += 1;
        if (params.adaptive.x > 0) {
            float l = lum(color), delta = l - moment.x;
            moment.x += delta / sum.w;
            moment.y += delta * (l - moment.x);
            if (sum.w >= float(params.config.w) && uint(sum.w) % 16 == 0) {
                float error = 1.96f * sqrt(moment.y / (sum.w - 1) / sum.w);
                if (error < params.adaptive.x * max(moment.x, .02f)) {
                    moment.z = 1;
                    break;
                }
            }
        }
    }
    film[id] = sum;
    moments[id] = moment;
}

// Navigation-only headlight preview. This kernel never contributes to the path-tracing film.
kernel void navigation_preview(
    device const Primitive *primitives [[buffer(0)]], device const Node *nodes [[buffer(1)]],
    device const Material *materials [[buffer(2)]], device float4 *film [[buffer(4)]],
    constant Params &params [[buffer(6)]], device const float4 *texels [[buffer(7)]],
    device const uint4 *textures [[buffer(8)]], device const IntersectionPrimitive *geometry [[buffer(9)]],
    NATIVE_BINDINGS uint local_id [[thread_position_in_grid]]) {
    NATIVE_CONTEXT
    if (local_id >= params.tile.z * params.tile.w) return;
    uint2 pixel = tile_coordinates(local_id, params.tile);
    uint x = pixel.x, y = pixel.y;
    Camera c = params.camera;
    float sx = 2 * (float(x) + .5f) / float(params.image.x) - 1;
    float sy = 1 - 2 * (float(y) + .5f) / float(params.image.y);
    Ray ray = {c.origin.xyz, normalize(c.forward.xyz + sx * c.right.w * c.right.xyz + sy * c.up.w * c.up.xyz)};
    float3 tint = 1, result = 0;
    bool resolved = false;
    for (uint depth = 0; depth < 8; depth++) {
        Hit h;
        if (!intersect(primitives, geometry, nodes, ray, INFINITY, false, h, materials, texels, textures NATIVE_ARGUMENT)) {
            result += tint * max(float3(.12f), environment_radiance(ray.d, params, texels, textures));
            resolved = true;
            break;
        }
        Material m = materials[h.material];
        float facing = abs(dot(h.n, -ray.d));
        if (m.surface.z > .5f) {
            float edge = 1 - facing;
            result += tint * (.025f + .20f * edge * edge * edge);
            tint *= .92f;
            ray.o = offset(h.p, h.g, ray.d);
            continue;
        }
        result += tint * (surface_color(m, h, texels, textures) * (.18f + .82f * facing) + min(m.emission.xyz, float3(2)));
        resolved = true;
        break;
    }
    if (!resolved) result += tint * .12f;
    film[y * params.image.x + x] = float4(result, 1);
}
