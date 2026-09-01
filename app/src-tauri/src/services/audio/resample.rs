/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation into a reusable buffer.
#[inline]
pub fn upsample_2x_into(input: &[f32], out: &mut Vec<f32>) {
    out.clear();
    if input.is_empty() {
        return;
    }
    let len = input.len();
    out.reserve(len * 2);
    for i in 0..len {
        let p1 = input[i];
        out.push(p1);

        let p0 = if i > 0 { input[i - 1] } else { p1 };
        let p2 = if i + 1 < len { input[i + 1] } else { p1 };
        let p3 = if i + 2 < len { input[i + 2] } else { p2 };

        let v = 0.5 * (p2 - p0);
        let v_next = if i + 2 < len {
            let p4 = if i + 3 < len { input[i + 3] } else { p3 };
            0.5 * (p4 - p1)
        } else {
            0.0
        };

        let interp = 0.5 * (p1 + p2) + 0.125 * (v - v_next);
        out.push(interp);
    }
}

/// Upsample 24kHz mono PCM to 48kHz via cubic Hermite interpolation.
#[inline]
pub fn upsample_2x(input: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(input.len() * 2);
    upsample_2x_into(input, &mut out);
    out
}
