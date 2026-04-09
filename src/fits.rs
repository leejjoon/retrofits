//! FITS file parsing and pixel data extraction.
//!
//! Provides [`FitsImage`] which holds the parsed header metadata and the
//! raw pixel data as a 2D `f32` array.

use anyhow::{Context, Result, bail};
use ndarray::Array2;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use fitsrs::{Fits, HDU, Pixels};

/// Parsed FITS image containing header metadata and pixel data.
#[derive(Debug)]
pub struct FitsImage {
    /// Header cards as key-value pairs.
    pub header: HashMap<String, String>,
    /// 2D pixel data, shape is (naxis2, naxis1) i.e. (rows, cols).
    pub data: Array2<f32>,
    /// Image width (NAXIS1).
    pub width: usize,
    /// Image height (NAXIS2).
    pub height: usize,
}

impl FitsImage {
    /// Minimum pixel value in the data array.
    pub fn min_value(&self) -> f32 {
        self.data.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// Maximum pixel value in the data array.
    pub fn max_value(&self) -> f32 {
        self.data
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
    }
}

/// Load a FITS file from disk, parse the primary HDU, and extract the
/// 2D image data as `f32` pixels.
///
/// Handles BITPIX values of 8, 16, 32, -32, and -64 by converting all
/// pixel types to `f32`. BZERO/BSCALE rescaling is applied if present
/// in the header.
pub fn load_fits(path: &Path) -> Result<FitsImage> {
    let f = File::open(path)
        .with_context(|| format!("Failed to open FITS file: {}", path.display()))?;
    let reader = BufReader::new(f);
    let mut hdu_list = Fits::from_reader(reader);

    // Get the primary HDU
    let hdu = hdu_list
        .next()
        .ok_or_else(|| anyhow::anyhow!("FITS file contains no HDUs"))?
        .with_context(|| "Failed to parse primary HDU")?;

    match hdu {
        HDU::Primary(primary) => {
            // Extract header metadata
            let xtension = primary.get_header().get_xtension();
            let mut header = HashMap::new();

            // Get axis dimensions
            let naxis = xtension.get_naxis();
            if naxis.len() < 2 {
                bail!(
                    "Expected 2D image (NAXIS >= 2), got NAXIS = {}",
                    naxis.len()
                );
            }

            let naxis1 = naxis[0] as usize; // width (columns)
            let naxis2 = naxis[1] as usize; // height (rows)

            header.insert("NAXIS1".to_string(), naxis1.to_string());
            header.insert("NAXIS2".to_string(), naxis2.to_string());
            header.insert("NAXIS".to_string(), naxis.len().to_string());

            // Extract BITPIX
            let bitpix = xtension.get_bitpix();
            header.insert("BITPIX".to_string(), format!("{:?}", bitpix));

            // Extract additional header cards
            for card in primary.get_header().cards() {
                match card {
                    fitsrs::card::Card::Value { name, value } 
                    | fitsrs::card::Card::Hierarch { name, value } => {
                        let val_str = match value {
                            fitsrs::card::Value::Integer { value: v, .. } => v.to_string(),
                            fitsrs::card::Value::Float { value: v, .. } => v.to_string(),
                            fitsrs::card::Value::Logical { value: v, .. } => v.to_string(),
                            fitsrs::card::Value::String { value: v, .. } => v.clone(),
                            fitsrs::card::Value::Undefined => String::new(),
                            fitsrs::card::Value::Invalid(v) => v.clone(),
                        };
                        header.insert(name.clone(), val_str);
                    }
                    _ => {}
                }
            }

            // Extract BZERO and BSCALE for rescaling
            let bzero: f64 = header
                .get("BZERO")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            let bscale: f64 = header
                .get("BSCALE")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(1.0);

            // Read pixel data
            let image_data = hdu_list.get_data(&primary);
            let total_pixels = naxis1 * naxis2;
            let mut pixels_f32 = Vec::with_capacity(total_pixels);

            match image_data.pixels() {
                Pixels::U8(it) => {
                    for val in it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
                Pixels::I16(it) => {
                    for val in it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
                Pixels::I32(it) => {
                    for val in it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
                Pixels::I64(it) => {
                    for val in it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
                Pixels::F32(it) => {
                    for val in it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
                Pixels::F64(it) => {
                    for val in it {
                        let rescaled = val * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                }
            }

            if pixels_f32.len() != total_pixels {
                bail!(
                    "Expected {} pixels ({}x{}), but read {}",
                    total_pixels,
                    naxis1,
                    naxis2,
                    pixels_f32.len()
                );
            }

            // Build Array2 with shape (rows=naxis2, cols=naxis1)
            let data = Array2::from_shape_vec((naxis2, naxis1), pixels_f32)
                .context("Failed to construct 2D array from pixel data")?;

            Ok(FitsImage {
                header,
                data,
                width: naxis1,
                height: naxis2,
            })
        }
        _ => bail!("Primary HDU is not an image extension"),
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn example_fits_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example_fits/18109J000.fits")
    }

    #[test]
    fn test_load_example_fits() {
        let result = load_fits(&example_fits_path());
        assert!(result.is_ok(), "Failed to load FITS: {:?}", result.err());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.shape(), &[img.height, img.width]);
    }

    #[test]
    fn test_header_extraction() {
        let img = load_fits(&example_fits_path()).unwrap();
        // Must have NAXIS, NAXIS1, NAXIS2
        assert!(img.header.contains_key("NAXIS"));
        assert!(img.header.contains_key("NAXIS1"));
        assert!(img.header.contains_key("NAXIS2"));
        // NAXIS should be 2
        assert_eq!(img.header["NAXIS"], "2");
        // Dimensions should match struct fields
        assert_eq!(img.header["NAXIS1"], img.width.to_string());
        assert_eq!(img.header["NAXIS2"], img.height.to_string());
    }

    #[test]
    fn test_pixel_range() {
        let img = load_fits(&example_fits_path()).unwrap();
        // All pixels should be finite f32
        assert!(img.data.iter().all(|v| v.is_finite()));
        // For a typical astronomical exposure, min should be >= 0
        // (though some calibrated data can go negative)
        let min = img.min_value();
        let max = img.max_value();
        assert!(min.is_finite());
        assert!(max.is_finite());
        assert!(max > min, "Image should have dynamic range");
    }

    #[test]
    fn test_invalid_path() {
        let result = load_fits(Path::new("/nonexistent/file.fits"));
        assert!(result.is_err());
    }
}
