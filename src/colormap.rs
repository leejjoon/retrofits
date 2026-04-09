//! Colormap application to map 0..=1 grayscale values into RGB colors.

use image::RgbaImage;
use ndarray::ArrayView2;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ColormapName {
    #[default]
    Grayscale,
    Viridis,
    Plasma,
    Inferno,
    Magma,
}

impl ColormapName {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Grayscale => Self::Viridis,
            Self::Viridis => Self::Plasma,
            Self::Plasma => Self::Inferno,
            Self::Inferno => Self::Magma,
            Self::Magma => Self::Grayscale,
        }
    }
}

/// Applies the selected colormap to the stretched data `[0.0, 1.0]`.
/// Returns an `RgbaImage` suitable for rendering.
pub fn apply_colormap(stretched: ArrayView2<f32>, cmap: ColormapName) -> RgbaImage {
    let (height, width) = (stretched.shape()[0] as u32, stretched.shape()[1] as u32);
    
    // Create an empty RgbaImage
    let mut img = RgbaImage::new(width, height);

    // Get the underlying buffer for parallel processing.
    // Note: RgbaImage::as_flat_samples_mut() could be used, or just zip rows.
    // An easy way to go parallel: process chunks of the raw buffer.
    
    let buffer: &mut [u8] = &mut img;
    
    // We can zip the flattened ndarray and the pixel buffer (4 bytes per pixel)
    // Unfortunately, if the ndarray is not contiguous, we have to iterate carefully.
    // We assume the extracted viewport or entire array is standard memory layout (C-contig).
    if let Some(slice) = stretched.as_slice() {
        buffer.par_chunks_exact_mut(4)
            .zip(slice.par_iter())
            .for_each(|(pixel, &val)| {
                // val is assumed to be clamped between 0.0 and 1.0
                let t = val; // colorous takes f64

                let (r, g, b) = match cmap {
                    ColormapName::Grayscale => {
                        let gray = (t * 255.0) as u8;
                        (gray, gray, gray)
                    }
                    ColormapName::Viridis => {
                        let c = colorous::VIRIDIS.eval_continuous(t as f64);
                        (c.r, c.g, c.b)
                    }
                    ColormapName::Plasma => {
                        let c = colorous::PLASMA.eval_continuous(t as f64);
                        (c.r, c.g, c.b)
                    }
                    ColormapName::Inferno => {
                        let c = colorous::INFERNO.eval_continuous(t as f64);
                        (c.r, c.g, c.b)
                    }
                    ColormapName::Magma => {
                        let c = colorous::MAGMA.eval_continuous(t as f64);
                        (c.r, c.g, c.b)
                    }
                };

                pixel[0] = r;
                pixel[1] = g;
                pixel[2] = b;
                pixel[3] = 255; // Alpha opaque
            });
    } else {
        // Fallback for non-contiguous views
        unimplemented!("Non-contiguous stretched view is not yet supported for colormap")
    }

    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_grayscale_colormap() {
        let input = ndarray::arr2(&[[0.0, 0.5], [1.0, 1.0]]);
        let img = apply_colormap(input.view(), ColormapName::Grayscale);
        
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
        
        // 0.0 -> black
        assert_eq!(img.get_pixel(0, 0), &Rgba([0, 0, 0, 255]));
        // 0.5 -> mid-gray
        assert_eq!(img.get_pixel(1, 0), &Rgba([127, 127, 127, 255]));
        // 1.0 -> white
        assert_eq!(img.get_pixel(0, 1), &Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_viridis_colormap() {
        let input = ndarray::arr2(&[[0.0, 1.0]]);
        let img = apply_colormap(input.view(), ColormapName::Viridis);
        
        // Colormaps in colorous: Viridis at 0.0 is dark purple (around [68, 1, 84])
        // Let's just check it's not Grayscale.
        let p0 = img.get_pixel(0, 0);
        let p1 = img.get_pixel(1, 0);
        
        assert_ne!(p0[0], p0[1]); // Not grayscale
        assert_ne!(p1[0], p1[1]); 
    }
}
