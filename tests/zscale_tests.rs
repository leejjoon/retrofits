use ndarray::Array2;
use retrofits::zscale::estimate_zscale;

#[test]
fn test_estimate_zscale_flat() {
    let data = Array2::from_elem((100, 100), 10.0f32);
    let (vmin, vmax) = estimate_zscale(&data, 0.25);
    // For a flat image, sigma is 0, so vmin and vmax should be the median (10.0)
    assert_eq!(vmin, 10.0);
    assert_eq!(vmax, 10.0);
}

#[test]
fn test_estimate_zscale_ramp() {
    // A ramp from 0 to 9999. ZScale fits the (perfectly linear) sorted
    // distribution and, after the contrast expansion, the window saturates to
    // the full data range — a ramp has no faint "background" to isolate.
    let mut data = Array2::zeros((100, 100));
    for (i, val) in data.iter_mut().enumerate() {
        *val = i as f32;
    }

    let (vmin, vmax) = estimate_zscale(&data, 0.25);

    let median = 4999.5;
    assert!(vmin < median);
    assert!(vmax > median);
    // Clamped to the actual data range [0, 9999].
    assert_eq!(vmin, 0.0);
    assert_eq!(vmax, 9999.0);
}
