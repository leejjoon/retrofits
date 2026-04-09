use ndarray::Array2;
use std::cmp::Ordering;

/// Estimate Z‑Scale cut values for an image.
///
/// * `data` – 2‑D array of pixel values (full image).
/// * `contrast` – contrast factor (IRAF default is 0.25).
///
/// The algorithm samples up to 10 000 pixels (or all if the image is smaller),
/// computes the median and standard deviation of the sample, and returns
/// `vmin = median - contrast * sigma` and `vmax = median + contrast * sigma`.
pub fn estimate_zscale(data: &Array2<f32>, contrast: f32) -> (f32, f32) {
    let total = data.len();
    let max_samples = 10_000usize;
    let step = if total > max_samples {
        (total / max_samples).max(1)
    } else {
        1
    };
    let mut samples: Vec<f32> = Vec::with_capacity(std::cmp::min(total, max_samples));
    for (_i, &v) in data.iter().enumerate().step_by(step) {
        samples.push(v);
    }
    // Median
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mid = samples.len() / 2;
    let median = if samples.len().is_multiple_of(2) {
        (samples[mid - 1] + samples[mid]) / 2.0
    } else {
        samples[mid]
    };
    // Standard deviation
    let mean: f32 = samples.iter().copied().sum::<f32>() / samples.len() as f32;
    let var: f32 = samples
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f32>()
        / samples.len() as f32;
    let sigma = var.sqrt();
    let vmin = median - contrast * sigma;
    let vmax = median + contrast * sigma;
    (vmin, vmax)
}
