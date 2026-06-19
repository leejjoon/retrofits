use ndarray::Array2;

// IRAF ZScale tuning constants (same defaults as IRAF/numdisplay/DS9).
const MIN_NPIXELS: usize = 5;
const MAX_REJECT: f32 = 0.5;
const KREJ: f32 = 2.5;
const MAX_ITERATIONS: usize = 5;

/// Estimate Z‑Scale cut values for an image.
///
/// * `data` – 2‑D array of pixel values (full image).
/// * `contrast` – contrast factor (IRAF default is 0.25).
///
/// This is the IRAF ZScale algorithm (as used by IRAF `display`, DS9, and
/// numdisplay): sample the image, sort the sample, then fit a straight line to
/// the sorted pixel values vs. their rank using iterative k‑sigma rejection.
/// The slope of that line near the median — divided by `contrast` — sets the
/// display window. Bright stars and other outliers are rejected during the fit,
/// so the faint background stays visible (unlike a naive min/max or
/// `median ± σ`).
pub fn estimate_zscale(data: &Array2<f32>, contrast: f32) -> (f32, f32) {
    let total = data.len();
    let max_samples = 10_000usize;
    let step = if total > max_samples {
        (total / max_samples).max(1)
    } else {
        1
    };
    let mut samples: Vec<f32> = Vec::with_capacity(std::cmp::min(total, max_samples));
    for &v in data.iter().step_by(step) {
        // Skip NaN/inf pixels (common in astronomical images): they have no
        // total order and would both panic the sort and poison the statistics.
        if v.is_finite() {
            samples.push(v);
        }
    }
    // Guard against an image that is entirely non-finite.
    if samples.is_empty() {
        return (0.0, 1.0);
    }
    // Sort the sample (all finite, so this is a true total order).
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let npix = samples.len();
    let zmin = samples[0];
    let zmax = samples[npix - 1];

    let center_pixel = (npix - 1) / 2;
    let median = if npix % 2 == 1 {
        samples[center_pixel]
    } else {
        0.5 * (samples[center_pixel] + samples[center_pixel + 1])
    };

    let minpix = MIN_NPIXELS.max((npix as f32 * MAX_REJECT) as usize);
    let ngrow = 1.max((npix as f32 * 0.01) as usize);
    let (ngoodpix, zslope) = fit_line(&samples, KREJ, ngrow, MAX_ITERATIONS);

    if ngoodpix < minpix {
        // Too few pixels survived the fit — fall back to the full range.
        (zmin, zmax)
    } else {
        let slope = if contrast > 0.0 {
            zslope / contrast
        } else {
            zslope
        };
        // Extrapolate the fitted slope out from the median to the ends of the
        // sample, clamped to the actual data range.
        let z1 = zmin.max(median - center_pixel as f32 * slope);
        let z2 = zmax.min(median + (npix - 1 - center_pixel) as f32 * slope);
        (z1, z2)
    }
}

/// Fit a straight line to the sorted sample (value vs. rank) with iterative
/// k‑sigma rejection of outliers. Returns the number of surviving ("good")
/// pixels and the slope expressed in value-per-rank units.
fn fit_line(samples: &[f32], krej: f32, ngrow: usize, maxiter: usize) -> (usize, f32) {
    let npix = samples.len();
    if npix < 2 {
        return (npix, 0.0);
    }

    // Normalize rank to x in [-1, 1] for a well-conditioned fit.
    let xscale = 2.0 / (npix as f32 - 1.0);
    let xnorm: Vec<f32> = (0..npix).map(|i| i as f32 * xscale - 1.0).collect();

    let minpix = MIN_NPIXELS.max((npix as f32 * MAX_REJECT) as usize);
    let mut badpix = vec![false; npix];
    let mut ngoodpix = npix;
    let mut last_ngoodpix = npix + 1;
    let mut slope = 0.0f32;

    for _ in 0..maxiter {
        if ngoodpix >= last_ngoodpix || ngoodpix < minpix {
            break;
        }
        last_ngoodpix = ngoodpix;

        // Least-squares line fit over the currently-good pixels (f64 accumulators).
        let (mut sum, mut sumx, mut sumxx, mut sumxy, mut sumy) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for i in 0..npix {
            if !badpix[i] {
                let x = xnorm[i] as f64;
                let y = samples[i] as f64;
                sum += 1.0;
                sumx += x;
                sumxx += x * x;
                sumxy += x * y;
                sumy += y;
            }
        }
        let delta = sum * sumxx - sumx * sumx;
        if delta == 0.0 {
            break;
        }
        let intercept = (sumxx * sumy - sumx * sumxy) / delta;
        slope = ((sum * sumxy - sumx * sumy) / delta) as f32;
        let intercept = intercept as f32;

        // Residuals from the fit, and their sigma over the good pixels.
        let (mut n, mut sflat, mut sflat2) = (0usize, 0f64, 0f64);
        for i in 0..npix {
            if !badpix[i] {
                let flat = (samples[i] - (xnorm[i] * slope + intercept)) as f64;
                n += 1;
                sflat += flat;
                sflat2 += flat * flat;
            }
        }
        if n == 0 {
            break;
        }
        let mean = sflat / n as f64;
        let sigma = ((sflat2 / n as f64 - mean * mean).max(0.0)).sqrt();
        let threshold = (sigma * krej as f64) as f32;
        let mean = mean as f32;

        // Reject pixels lying more than k*sigma from the fitted line.
        for i in 0..npix {
            let flat = samples[i] - (xnorm[i] * slope + intercept);
            if flat < mean - threshold || flat > mean + threshold {
                badpix[i] = true;
            }
        }
        // Grow rejected regions (matches numpy.convolve(..., 'same') dilation).
        if ngrow > 1 {
            badpix = grow_rejections(&badpix, ngrow);
        }

        ngoodpix = badpix.iter().filter(|&&b| !b).count();
    }

    // Convert slope from normalized-x units back to per-rank units.
    (ngoodpix, slope * xscale)
}

/// Dilate the rejection mask by a kernel of length `ngrow`, equivalent to
/// `numpy.convolve(mask, ones(ngrow), mode="same") > 0`.
fn grow_rejections(badpix: &[bool], ngrow: usize) -> Vec<bool> {
    let n = badpix.len();
    let right = (ngrow - 1) / 2;
    let left = (ngrow - 1) - right;
    let mut out = vec![false; n];
    for i in 0..n {
        let lo = i.saturating_sub(left);
        let hi = (i + right).min(n - 1);
        out[i] = badpix[lo..=hi].iter().any(|&b| b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn flat_image_collapses_to_constant() {
        let data = Array2::from_elem((100, 100), 10.0f32);
        let (vmin, vmax) = estimate_zscale(&data, 0.25);
        assert_eq!(vmin, 10.0);
        assert_eq!(vmax, 10.0);
    }

    #[test]
    fn rejects_bright_outliers() {
        // A faint linear-gradient background (95..=105) with a handful of very
        // bright "star" pixels. ZScale should reject the stars and produce a
        // window near the background — nowhere near the 5000-count outliers
        // that a naive min/max would surface.
        let mut data = Array2::<f32>::zeros((100, 100));
        let n = data.len();
        for (i, val) in data.iter_mut().enumerate() {
            *val = 95.0 + 10.0 * (i as f32) / (n as f32 - 1.0);
        }
        for i in 0..10 {
            data[[0, i]] = 5000.0;
        }

        let (vmin, vmax) = estimate_zscale(&data, 0.25);

        // Window brackets the background median (~100) and excludes the stars.
        assert!(vmin >= 90.0 && vmin <= 100.0, "vmin = {vmin}");
        assert!(vmax >= 100.0 && vmax < 500.0, "vmax = {vmax}");
    }

    #[test]
    fn handles_nan_pixels_without_panic() {
        let mut a = Array2::<f32>::zeros((4, 4));
        a[[0, 0]] = f32::NAN;
        a[[1, 1]] = f32::INFINITY;
        a[[2, 2]] = 10.0;
        let (vmin, vmax) = estimate_zscale(&a, 0.25);
        assert!(vmin.is_finite() && vmax.is_finite());
    }

    #[test]
    fn all_nan_falls_back() {
        let a = Array2::<f32>::from_elem((3, 3), f32::NAN);
        let (vmin, vmax) = estimate_zscale(&a, 0.25);
        assert_eq!((vmin, vmax), (0.0, 1.0));
    }
}
