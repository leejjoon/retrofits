//! Mathematical array stretching (Linear, Logarithmic, Asinh) for astrophotography.

use ndarray::{Array2, ArrayView2};


#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StretchFunction {
    Linear,
    Logarithmic,
    #[default]
    Asinh,
}

/// Applies a non-linear stretch to the input data, mapping values from `[black_point, white_point]`
/// into the `[0.0, 1.0]` range.
///
/// Uses parallel iteration across rows (or chunks) to speed up processing.
pub fn compute_stretch(
    data: ArrayView2<f32>,
    func: StretchFunction,
    black_point: f32,
    white_point: f32,
) -> Array2<f32> {
    let range = white_point - black_point;
    let range = if range <= 0.0 { f32::EPSILON } else { range };

    let mut result = Array2::zeros(data.raw_dim());

    // We use rayon to parallel-process the internal slices.
    // ndarray's Zip or par_mapv could also be used, but since we map to a new array,
    // Zip::from(&mut result).and(&data).par_for_each is efficient.
    ndarray::Zip::from(&mut result)
        .and(&data)
        .par_for_each(|out, &val| {
            // Normalize to 0..1 range first
            let mut normalized = (val - black_point) / range;
            
            // Clamp strictly before complex math
            if normalized < 0.0 {
                normalized = 0.0;
            } else if normalized > 1.0 {
                normalized = 1.0;
            }

            *out = match func {
                StretchFunction::Linear => normalized,
                StretchFunction::Logarithmic => {
                    // Log stretch: log(a * x + 1) / log(a + 1)
                    let a = 1000.0_f32; // stretch intensity
                    (a * normalized + 1.0).ln() / (a + 1.0).ln()
                }
                StretchFunction::Asinh => {
                    // Asinh stretch: asinh(a * x) / asinh(a)
                    let a = 1000.0_f32; // stretch intensity
                    (a * normalized).asinh() / (a).asinh()
                }
            };
        });

    result
}

/// Calculate auto-stretch parameters based on percentiles.
/// Returns (black_point, white_point).
pub fn auto_stretch_params(data: ArrayView2<f32>) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    
    // Quick min/max pass (for a real app, you might want to compute
    // actual percentiles via a histogram for better auto-stretch).
    for &val in data.iter() {
        if val.is_finite() {
            if val < min { min = val; }
            if val > max { max = val; }
        }
    }
    
    // Simple heuristic: set black point to min, white point to 99% of max if min=0
    let range = max - min;
    let black = min;
    let white = min + range * 0.99; // Clip top 1% blindly
    
    (black, white)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_stretch() {
        let input = ndarray::arr2(&[[0.0, 50.0], [100.0, 150.0]]);
        let out = compute_stretch(input.view(), StretchFunction::Linear, 0.0, 100.0);
        
        assert_eq!(out[[0, 0]], 0.0);
        assert_eq!(out[[0, 1]], 0.5);
        assert_eq!(out[[1, 0]], 1.0);
        assert_eq!(out[[1, 1]], 1.0); // Clamped
    }

    #[test]
    fn test_log_stretch() {
        let input = ndarray::arr2(&[[0.0, 10.0], [50.0, 100.0]]);
        let out = compute_stretch(input.view(), StretchFunction::Logarithmic, 0.0, 100.0);
        
        assert_eq!(out[[0, 0]], 0.0);
        assert_eq!(out[[1, 1]], 1.0);
        // Log curve should raise 10% (0.1) significantly higher than 0.1
        assert!(out[[0, 1]] > 0.1);
    }

    #[test]
    fn test_asinh_stretch() {
        let input = ndarray::arr2(&[[0.0, 10.0], [50.0, 100.0]]);
        let out = compute_stretch(input.view(), StretchFunction::Asinh, 0.0, 100.0);
        
        assert_eq!(out[[0, 0]], 0.0);
        assert_eq!(out[[1, 1]], 1.0);
        // Asinh should also compress highlights
        assert!(out[[0, 1]] > 0.1);
    }
}
