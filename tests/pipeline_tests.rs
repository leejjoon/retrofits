use retrofits::colormap::{apply_colormap, ColormapName};
use retrofits::stretch::{auto_stretch_params, compute_stretch, StretchFunction};

#[test]
fn test_stretch_auto_minmax() {
    let input = ndarray::arr2(&[[0.0, 50.0], [100.0, 200.0]]);
    let (black, white) = auto_stretch_params(input.view());
    assert_eq!(black, 0.0);
    // 0 + 200 * 0.99 = 198.0
    assert_eq!(white, 198.0);
}

#[test]
fn test_full_pipeline() {
    // A small 4x4 image
    let data = vec![
        0.0f32, 10.0, 20.0, 30.0,
        40.0,   50.0, 60.0, 70.0,
        80.0,   90.0, 100.0, 110.0,
        120.0, 130.0, 140.0, 150.0,
    ];
    let input = ndarray::Array2::from_shape_vec((4, 4), data).unwrap();

    let (black, white) = auto_stretch_params(input.view());
    
    // Stretch
    let stretched = compute_stretch(input.view(), StretchFunction::Asinh, black, white);
    assert_eq!(stretched.shape(), &[4, 4]);

    // Apply colormap
    let img = apply_colormap(stretched.view(), ColormapName::Viridis);
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}
