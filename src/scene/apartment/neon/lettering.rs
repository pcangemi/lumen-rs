//! Original slanted script centre lines, drawn for bent glass rather than font outlines.
use glam::Vec2;

type Cubic = [[f32; 2]; 4];

fn glyph(letter: char) -> (f32, Vec<Cubic>) {
    match letter {
        'C' => (
            0.80,
            vec![
                [[0.72, 0.73], [1.02, 1.07], [0.38, 1.17], [0.14, 0.67]],
                [[0.14, 0.67], [-0.15, 0.08], [0.12, -0.18], [0.48, 0.04]],
                [[0.48, 0.04], [0.60, 0.11], [0.71, 0.23], [0.76, 0.33]],
            ],
        ),
        'a' => (
            0.62,
            vec![
                [[0.39, 0.48], [0.19, 0.66], [-0.02, 0.31], [0.06, 0.12]],
                [[0.06, 0.12], [0.15, -0.09], [0.38, 0.13], [0.43, 0.47]],
                [[0.43, 0.47], [0.45, 0.62], [0.48, 0.60], [0.45, 0.43]],
                [[0.45, 0.43], [0.38, -0.04], [0.45, -0.02], [0.61, 0.14]],
            ],
        ),
        'n' => (
            0.65,
            vec![
                [[0.0, 0.12], [0.08, 0.23], [0.14, 0.41], [0.14, 0.56]],
                [[0.14, 0.56], [0.16, 0.60], [0.15, 0.31], [0.12, 0.04]],
                [[0.12, 0.04], [0.25, 0.42], [0.51, 0.77], [0.49, 0.40]],
                [[0.49, 0.40], [0.43, -0.02], [0.48, -0.02], [0.64, 0.14]],
            ],
        ),
        'g' => (
            0.62,
            vec![
                [[0.38, 0.48], [0.18, 0.67], [-0.03, 0.28], [0.07, 0.10]],
                [[0.07, 0.10], [0.19, -0.07], [0.39, 0.19], [0.43, 0.48]],
                [[0.43, 0.48], [0.49, 0.76], [0.44, 0.09], [0.34, -0.28]],
                [[0.34, -0.28], [0.21, -0.72], [-0.12, -0.44], [0.14, -0.24]],
                [[0.14, -0.24], [0.33, -0.10], [0.48, -0.01], [0.61, 0.14]],
            ],
        ),
        'e' => (
            0.53,
            vec![
                [[0.0, 0.12], [0.28, 0.22], [0.46, 0.46], [0.27, 0.53]],
                [[0.27, 0.53], [0.07, 0.59], [-0.03, 0.17], [0.13, 0.07]],
                [[0.13, 0.07], [0.24, -0.02], [0.41, 0.05], [0.52, 0.16]],
            ],
        ),
        'm' => (
            0.91,
            vec![
                [[0.0, 0.12], [0.06, 0.23], [0.13, 0.39], [0.13, 0.56]],
                [[0.13, 0.56], [0.15, 0.60], [0.14, 0.26], [0.11, 0.04]],
                [[0.11, 0.04], [0.25, 0.45], [0.45, 0.70], [0.40, 0.36]],
                [[0.40, 0.36], [0.39, 0.26], [0.37, 0.12], [0.35, 0.04]],
                [[0.35, 0.04], [0.50, 0.46], [0.76, 0.72], [0.71, 0.36]],
                [[0.71, 0.36], [0.65, -0.03], [0.73, -0.02], [0.89, 0.14]],
            ],
        ),
        '\'' => (
            0.22,
            vec![[[0.09, 0.88], [0.16, 0.88], [0.13, 0.70], [0.02, 0.64]]],
        ),
        's' => (
            0.49,
            vec![
                [[0.01, 0.12], [0.15, 0.20], [0.33, 0.47], [0.30, 0.59]],
                [[0.30, 0.59], [0.24, 0.69], [0.18, 0.46], [0.27, 0.31]],
                [[0.27, 0.31], [0.52, -0.09], [-0.01, -0.07], [0.10, 0.10]],
                [[0.10, 0.10], [0.18, 0.17], [0.32, -0.02], [0.48, 0.14]],
            ],
        ),
        _ => unreachable!("The fixed inscription only uses its defined glyphs"),
    }
}

/// Sample the original centre lines, then apply a consistent italic slant while keeping the baseline horizontal.
/// No font file, traced logo or raster light mask is used.
pub(super) fn strokes() -> Vec<Vec<Vec2>> {
    let mut result = Vec::new();
    let mut advance = 0.0;
    for letter in "Cangem'sss".chars() {
        let (width, curves) = glyph(letter);
        let mut points = Vec::new();
        for (index, curve) in curves.into_iter().enumerate() {
            let p = curve.map(Vec2::from_array);
            let length: f32 = p.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
            let steps = (length / 0.009).ceil().max(8.0) as usize;
            for step in usize::from(index > 0)..=steps {
                let t = step as f32 / steps as f32;
                let s = 1.0 - t;
                let point = p[0] * s.powi(3)
                    + p[1] * (3.0 * s * s * t)
                    + p[2] * (3.0 * s * t * t)
                    + p[3] * t.powi(3);
                points.push(Vec2::new(advance + point.x + 0.32 * point.y, point.y));
            }
        }
        result.push(points);
        advance += width + 0.045;
    }
    // A separate sweeping underline echoes hand-painted Miami signage.
    let p = [
        Vec2::new(0.48, -0.60),
        Vec2::new(1.5, -0.53),
        Vec2::new(3.75, -0.53),
        Vec2::new(advance - 0.12, -0.60),
    ];
    result.push(
        (0..=360)
            .map(|i| {
                let t = i as f32 / 360.0;
                let s = 1.0 - t;
                p[0] * s.powi(3)
                    + p[1] * (3.0 * s * s * t)
                    + p[2] * (3.0 * s * t * t)
                    + p[3] * t.powi(3)
            })
            .collect(),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_letters_and_underline_keep_a_level_baseline() {
        let strokes = strokes();
        for pair in strokes[7..10].windows(2) {
            for (a, b) in pair[0].iter().zip(&pair[1]) {
                assert_eq!(a.y, b.y, "Successive letters must not climb the wall");
                assert!(b.x > a.x);
            }
        }
        let swash = strokes.last().unwrap();
        assert_eq!(swash[0].y, swash.last().unwrap().y);
    }
}
