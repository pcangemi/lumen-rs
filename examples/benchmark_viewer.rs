//! Measure unchanged-frame drawing separately from light transport and filtering.
use glam::Vec3;
use lumen_rs::viewer_ui::{FrameCache, ViewerState, draw, draw_cached};
use std::{hint::black_box, time::Instant};

fn main() {
    let mut state = ViewerState::new(1920, 1080, 2048, 50, "interior-night", 0.0);
    state.filtered_pixels = Some(
        (0..1920 * 1080)
            .map(|i| Vec3::new((i % 1920) as f32 / 1920.0, (i / 1920) as f32 / 1080.0, 0.3))
            .collect(),
    );
    state.status = "RENDERING".into();
    let mut cache = FrameCache::default();
    assert_eq!(
        draw(&state, 1280, 820).0,
        draw_cached(&state, 1280, 820, &mut cache).0
    );
    let frames = 300;
    let start = Instant::now();
    for _ in 0..frames {
        black_box(draw(black_box(&state), 1280, 820));
    }
    let uncached_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    for _ in 0..frames {
        black_box(draw_cached(black_box(&state), 1280, 820, &mut cache));
    }
    let cached_seconds = start.elapsed().as_secs_f64();
    println!(
        "{}",
        serde_json::json!({
            "frames": frames,
            "image": [1920, 1080],
            "viewport": [1280, 820],
            "uncached_seconds": uncached_seconds,
            "cached_seconds": cached_seconds,
            "draw_speedup": uncached_seconds / cached_seconds,
            "note": "Unchanged clean preview, exact frame equality. This measures CPU drawing, not render speed."
        })
    );
}
