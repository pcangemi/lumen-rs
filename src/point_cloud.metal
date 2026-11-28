#include <metal_stdlib>
using namespace metal;
struct Camera { float4 origin, forward, right, up; };
struct Point { float4 position, color; };
struct Vertex { float4 position [[position]]; float size [[point_size]]; float4 color; };
vertex Vertex cloud_vertex(uint id [[vertex_id]], device const Point *points [[buffer(0)]], constant Camera &camera [[buffer(1)]], constant float2 &style [[buffer(2)]]) {
    float3 relative = points[id].position.xyz - camera.origin.xyz;
    float depth = dot(relative, camera.forward.xyz);
    Vertex out;
    out.position = float4(dot(relative, camera.right.xyz) / camera.right.w,
                          dot(relative, camera.up.xyz) / camera.up.w,
                          (100.0f / (100.0f - .03f) - (100.0f * .03f / (100.0f - .03f)) / max(depth - style.y, .031f)) * depth, depth);
    out.size = clamp(2.0f - depth * .025f, 1.2f, 2.0f) * style.x;
    out.color = float4(sqrt(points[id].color.xyz) / (1.0f + max(depth, 0.0f) * .045f), 1);
    if (points[id].color.w == 0) out.position = float4(2, 2, 2, 1);
    if (style.x > 1.0f) out.color.xyz *= .22f;
    return out;
}
fragment float4 cloud_fragment(Vertex in [[stage_in]], float2 coordinate [[point_coord]]) {
    if (distance(coordinate, float2(.5f)) > .5f) discard_fragment();
    return in.color;
}
