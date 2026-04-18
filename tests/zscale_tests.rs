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
    // A ramp from 0 to 9999
    let mut data = Array2::zeros((100, 100));
    for (i, val) in data.iter_mut().enumerate() {
        *val = i as f32;
    }

    let (vmin, vmax) = estimate_zscale(&data, 0.25);

    let median = 4999.5;
    // Mean is roughly 5000, variance of a uniform distribution is approx (N^2)/12
    // std dev is roughly sqrt(10000^2 / 12) ~ 2886
    // vmin = 5000 - 0.25 * 2886 ~ 4278
    // vmax = 5000 + 0.25 * 2886 ~ 5721

    assert!(vmin < median);
    assert!(vmax > median);
    assert!(vmin > 0.0);
    assert!(vmax < 10000.0);
}
