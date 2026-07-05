//! FITS file parsing and pixel data extraction.
//!
//! Provides [`FitsImage`] which holds the parsed header metadata and the
//! raw pixel data as a 2D `f32` array.

use anyhow::{bail, Context, Result};
use ndarray::Array2;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use fitsrs::{Fits, Pixels, HDU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HduKind {
    Image,
    BinaryTable,
    AsciiTable,
}

impl std::fmt::Display for HduKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image => write!(f, "IMAGE"),
            Self::BinaryTable => write!(f, "BINTABLE"),
            Self::AsciiTable => write!(f, "TABLE"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub index: usize,
    pub name: String,
    pub is_image: bool,
    pub kind: HduKind,
    /// NAXIS dimensions (empty for tables).
    pub dims: Vec<usize>,
    /// Pixel type (e.g. "i16", "f32"); images only.
    pub bitpix: Option<String>,
}

impl ExtensionInfo {
    /// Short human-readable description for the extension picker.
    pub fn describe(&self) -> String {
        if self.is_image {
            let dims = self
                .dims
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\u{d7}");
            format!("{} {}", dims, self.bitpix.as_deref().unwrap_or(""))
        } else {
            "(not viewable)".to_string()
        }
    }
}

/// Structural description of an HDU for the extension list.
fn describe_hdu(hdu: &HDU) -> (HduKind, Vec<usize>, Option<String>) {
    match hdu {
        HDU::Primary(img) | HDU::XImage(img) => {
            let x = img.get_header().get_xtension();
            let dims: Vec<usize> = x.get_naxis().iter().map(|&d| d as usize).collect();
            let bitpix = Some(format!("{:?}", x.get_bitpix()).to_lowercase());
            (HduKind::Image, dims, bitpix)
        }
        HDU::XBinaryTable(_) => (HduKind::BinaryTable, Vec::new(), None),
        HDU::XASCIITable(_) => (HduKind::AsciiTable, Vec::new(), None),
    }
}

/// A single FITS header card, order-preserving and comment-preserving.
#[derive(Debug, Clone, PartialEq)]
pub enum HeaderEntry {
    /// `KEYWORD = value / comment`
    Value {
        keyword: String,
        value: String,
        comment: Option<String>,
    },
    /// A `COMMENT` card.
    Comment(String),
    /// A `HISTORY` card.
    History(String),
    /// A blank card.
    Blank,
}

impl HeaderEntry {
    /// The card rendered as a single text line. This is the exact text the
    /// header viewer displays and searches, so the two cannot disagree.
    pub fn display_text(&self) -> String {
        match self {
            HeaderEntry::Value {
                keyword,
                value,
                comment,
            } => match comment {
                Some(c) => format!("{:<8}= {} / {}", keyword, value, c),
                None => format!("{:<8}= {}", keyword, value),
            },
            HeaderEntry::Comment(s) => format!("COMMENT {}", s),
            HeaderEntry::History(s) => format!("HISTORY {}", s),
            HeaderEntry::Blank => String::new(),
        }
    }
}

/// A full FITS header: every card in file order.
#[derive(Debug, Clone, Default)]
pub struct FitsHeader(pub Vec<HeaderEntry>);

impl FitsHeader {
    /// Look up the value of a keyword (first occurrence). Headers are at most
    /// a few hundred cards, so a linear scan is fine.
    pub fn get(&self, kw: &str) -> Option<&str> {
        self.0.iter().find_map(|e| match e {
            HeaderEntry::Value { keyword, value, .. } if keyword == kw => Some(value.as_str()),
            _ => None,
        })
    }

    /// Insert a keyword at the front unless it is already present. Used to
    /// guarantee structural keywords (NAXIS/BITPIX) exist even when the
    /// parser does not surface them as regular cards.
    fn ensure_front(&mut self, kw: &str, value: String) {
        if self.get(kw).is_none() {
            self.0.insert(
                0,
                HeaderEntry::Value {
                    keyword: kw.to_string(),
                    value,
                    comment: None,
                },
            );
        }
    }
}

/// Extract a card value as a display string plus its optional comment.
fn value_to_strings(value: &fitsrs::card::Value) -> (String, Option<String>) {
    use fitsrs::card::Value as V;
    match value {
        V::Integer { value, comment } => (value.to_string(), comment.clone()),
        V::Float { value, comment } => (value.to_string(), comment.clone()),
        V::Logical { value, comment } => (
            if *value { "T" } else { "F" }.to_string(),
            comment.clone(),
        ),
        V::String { value, comment } => (value.clone(), comment.clone()),
        V::Undefined => (String::new(), None),
        V::Invalid(v) => (v.clone(), None),
    }
}

/// Parsed FITS image containing header metadata and pixel data.
#[derive(Debug)]
pub struct FitsImage {
    /// All header cards, in file order.
    pub header: FitsHeader,
    /// 2D pixel data, shape is (naxis2, naxis1) i.e. (rows, cols).
    pub data: Array2<f32>,
    /// Image width (NAXIS1).
    pub width: usize,
    /// Image height (NAXIS2).
    pub height: usize,
    /// All extensions present in the file.
    pub extensions: Vec<ExtensionInfo>,
    /// Currently loaded extension index.
    pub current_extension: usize,
    /// File path this was loaded from
    pub file_path: std::path::PathBuf,
}

impl FitsImage {
    /// Minimum pixel value in the data array.
    pub fn min_value(&self) -> f32 {
        self.data.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// Maximum pixel value in the data array.
    pub fn max_value(&self) -> f32 {
        self.data.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }
}

fn is_image_hdu(hdu: &HDU) -> bool {
    match hdu {
        HDU::Primary(img) | HDU::XImage(img) => {
            let naxis = img.get_header().get_xtension().get_naxis();
            naxis.len() >= 2 && naxis[0] > 0 && naxis[1] > 0
        }
        _ => false,
    }
}

fn ext_name_from_hdu(hdu: &HDU) -> String {
    match hdu {
        HDU::Primary(img) | HDU::XImage(img) => {
            img.get_header().get_parsed::<String>("EXTNAME").unwrap_or_default()
        }
        HDU::XBinaryTable(table) => table.get_header().get_parsed::<String>("EXTNAME").unwrap_or_default(),
        _ => String::new(),
    }
}

/// Reverse rows vertically in-place (FITS stores bottom-to-top, we render
/// top-to-bottom).
fn flip_rows(pixels: &mut [f32], width: usize, height: usize) {
    for i in 0..height / 2 {
        let top_idx = i * width;
        let bottom_idx = (height - 1 - i) * width;
        for j in 0..width {
            pixels.swap(top_idx + j, bottom_idx + j);
        }
    }
}

/// Load a FITS file from disk, parse the specified HDU, and extract the
/// 2D image data as `f32` pixels.
///
/// Handles BITPIX values of 8, 16, 32, -32, and -64 by converting all
/// pixel types to `f32`. BZERO/BSCALE rescaling is applied if present
/// in the header.
pub fn load_fits(path: &Path, ext_arg: Option<&str>) -> Result<FitsImage> {
    let f = File::open(path)
        .with_context(|| format!("Failed to open FITS file: {}", path.display()))?;
    let reader = BufReader::new(f);
    let hdu_list = Fits::from_reader(reader);

    let mut extensions = Vec::new();
    let mut target_hdu = None;
    let mut target_index = 0;

    let ext_arg_index = ext_arg.and_then(|s| s.parse::<usize>().ok());

    for (i, hdu_result) in hdu_list.enumerate() {
        let hdu = hdu_result.with_context(|| format!("Failed to parse HDU {}", i))?;
        let is_image = is_image_hdu(&hdu);
        let name = ext_name_from_hdu(&hdu);
        let (kind, dims, bitpix) = describe_hdu(&hdu);

        extensions.push(ExtensionInfo {
            index: i,
            name: name.clone(),
            is_image,
            kind,
            dims,
            bitpix,
        });

        if target_hdu.is_none() {
            if let Some(arg) = ext_arg {
                if (Some(i) == ext_arg_index || name == arg) && is_image {
                    target_hdu = Some(hdu);
                    target_index = i;
                }
            } else if is_image {
                target_hdu = Some(hdu);
                target_index = i;
            }
        }
    }

    let _ = target_hdu.ok_or_else(|| {
        if let Some(arg) = ext_arg {
            anyhow::anyhow!("Specified extension '{}' not found or is not an image", arg)
        } else {
            anyhow::anyhow!("No image extensions found in FITS file")
        }
    })?;

    // We must reopen to read pixel data because we consumed the iterator
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let mut hdu_list = Fits::from_reader(reader);
    for _ in 0..target_index {
        hdu_list.next();
    }
    let hdu = hdu_list.next().unwrap().unwrap();

    match hdu {
        HDU::Primary(img) | HDU::XImage(img) => {
            // Extract header metadata
            let xtension = img.get_header().get_xtension();

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

            // Collect every header card in file order, preserving comments,
            // COMMENT/HISTORY cards and blank lines.
            use fitsrs::card::Card;
            let mut header = FitsHeader::default();
            for card in img.get_header().cards() {
                match card {
                    Card::Value { name, value } | Card::Hierarch { name, value } => {
                        let (value, comment) = value_to_strings(value);
                        header.0.push(HeaderEntry::Value {
                            keyword: name.clone(),
                            value,
                            comment,
                        });
                    }
                    Card::Continuation { string, comment } => {
                        // Long-string convention: append to the previous card.
                        if let Some(HeaderEntry::Value {
                            value,
                            comment: prev_comment,
                            ..
                        }) = header.0.last_mut()
                        {
                            if value.ends_with('&') {
                                value.pop();
                            }
                            if let Some(s) = string {
                                value.push_str(s);
                            }
                            if prev_comment.is_none() {
                                prev_comment.clone_from(comment);
                            }
                        }
                    }
                    Card::Xtension { x, comment } => {
                        header.0.push(HeaderEntry::Value {
                            keyword: "XTENSION".to_string(),
                            value: x.to_string(),
                            comment: comment.clone(),
                        });
                    }
                    Card::Comment(s) => header.0.push(HeaderEntry::Comment(s.clone())),
                    Card::History(s) => header.0.push(HeaderEntry::History(s.clone())),
                    Card::Space => header.0.push(HeaderEntry::Blank),
                    Card::Undefined(s) => header.0.push(HeaderEntry::Comment(s.clone())),
                    Card::End => {}
                }
            }

            // Guarantee the structural keywords are queryable even if the
            // parser consumed them separately from the card list.
            let bitpix = xtension.get_bitpix();
            header.ensure_front("BITPIX", format!("{:?}", bitpix));
            header.ensure_front("NAXIS2", naxis2.to_string());
            header.ensure_front("NAXIS1", naxis1.to_string());
            header.ensure_front("NAXIS", naxis.len().to_string());

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
            let image_data = hdu_list.get_data(&img);
            let total_pixels = naxis1 * naxis2;
            let mut pixels_f32 = Vec::with_capacity(total_pixels);

            macro_rules! read_pixels {
                ($it:expr) => {
                    for val in $it {
                        let rescaled = (val as f64) * bscale + bzero;
                        pixels_f32.push(rescaled as f32);
                    }
                };
            }
            match image_data.pixels() {
                Pixels::U8(it) => read_pixels!(it),
                Pixels::I16(it) => read_pixels!(it),
                Pixels::I32(it) => read_pixels!(it),
                Pixels::I64(it) => read_pixels!(it),
                Pixels::F32(it) => read_pixels!(it),
                Pixels::F64(it) => read_pixels!(it),
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

            flip_rows(&mut pixels_f32, naxis1, naxis2);

            // Build Array2 with shape (rows=naxis2, cols=naxis1)
            let data = Array2::from_shape_vec((naxis2, naxis1), pixels_f32)
                .context("Failed to construct 2D array from pixel data")?;

            Ok(FitsImage {
                header,
                data,
                width: naxis1,
                height: naxis2,
                extensions,
                current_extension: target_index,
                file_path: path.to_path_buf(),
            })
        }
        _ => bail!("HDU is not an image extension"),
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
        let result = load_fits(&example_fits_path(), None);
        assert!(result.is_ok(), "Failed to load FITS: {:?}", result.err());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.shape(), &[img.height, img.width]);
    }

    #[test]
    fn test_header_extraction() {
        let img = load_fits(&example_fits_path(), None).unwrap();
        // Must have NAXIS, NAXIS1, NAXIS2
        assert!(img.header.get("NAXIS").is_some());
        assert!(img.header.get("NAXIS1").is_some());
        assert!(img.header.get("NAXIS2").is_some());
        // NAXIS should be 2
        assert_eq!(img.header.get("NAXIS"), Some("2"));
        // Dimensions should match struct fields
        assert_eq!(img.header.get("NAXIS1"), Some(img.width.to_string().as_str()));
        assert_eq!(img.header.get("NAXIS2"), Some(img.height.to_string().as_str()));
    }

    #[test]
    fn test_header_order_and_fidelity() {
        let img = load_fits(&example_fits_path(), None).unwrap();
        // The header must be non-trivial and ordered: SIMPLE (or XTENSION)
        // is required by the FITS standard to be the first card of an HDU.
        assert!(img.header.0.len() > 4);
        let first_value = img.header.0.iter().find_map(|e| match e {
            HeaderEntry::Value { keyword, .. } => Some(keyword.as_str()),
            _ => None,
        });
        assert!(
            matches!(first_value, Some("SIMPLE") | Some("XTENSION") | Some("NAXIS")),
            "unexpected first header keyword: {:?}",
            first_value
        );
        // display_text renders keyword = value
        let line = img.header.0[0].display_text();
        assert!(!line.is_empty());
    }

    #[test]
    fn test_pixel_range() {
        let img = load_fits(&example_fits_path(), None).unwrap();
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
        let result = load_fits(Path::new("/nonexistent/file.fits"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_specific_extension() {
        let result = load_fits(&example_fits_path(), Some("0"));
        assert!(result.is_ok(), "Failed to load FITS extension 0: {:?}", result.err());
    }

    #[test]
    fn test_load_invalid_extension() {
        let result = load_fits(&example_fits_path(), Some("999"));
        assert!(result.is_err());
    }
}
