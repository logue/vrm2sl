use image::{DynamicImage, imageops::FilterType};

/// Interpolation method used for texture resizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResizeInterpolation {
    /// Fast nearest-neighbor interpolation.
    Nearest,
    /// Bilinear interpolation (`image`'s `Triangle` filter).
    #[default]
    Bilinear,
    /// Bicubic interpolation (`image`'s `CatmullRom` filter).
    Bicubic,
    /// Gaussian interpolation.
    Gaussian,
    /// Lanczos (windowed sinc) interpolation.
    Lanczos3,
}

impl From<ResizeInterpolation> for FilterType {
    fn from(value: ResizeInterpolation) -> Self {
        match value {
            ResizeInterpolation::Nearest => FilterType::Nearest,
            ResizeInterpolation::Bilinear => FilterType::Triangle,
            ResizeInterpolation::Bicubic => FilterType::CatmullRom,
            ResizeInterpolation::Gaussian => FilterType::Gaussian,
            ResizeInterpolation::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

/// Resize image to fit within the specified max size while preserving aspect ratio.
///
/// If the image is already smaller than both limits, this function returns an
/// unchanged clone and does not upscale.
pub fn resize_texture_to_max(
    image: &DynamicImage,
    max_width: u32,
    max_height: u32,
    interpolation: ResizeInterpolation,
) -> DynamicImage {
    if image.width() <= max_width && image.height() <= max_height {
        return image.clone();
    }

    image.resize(max_width, max_height, interpolation.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbaImage};

    #[test]
    fn given_large_texture_when_resize_with_lanczos_then_image_fits_bounds() {
        let source = DynamicImage::ImageRgba8(RgbaImage::new(2048, 1024));

        let resized = resize_texture_to_max(&source, 1024, 1024, ResizeInterpolation::Lanczos3);

        assert_eq!(resized.dimensions(), (1024, 512));
    }

    #[test]
    fn given_small_texture_when_resize_then_original_size_is_kept() {
        let source = DynamicImage::ImageRgba8(RgbaImage::new(512, 512));

        let resized = resize_texture_to_max(&source, 1024, 1024, ResizeInterpolation::Bilinear);

        assert_eq!(resized.dimensions(), (512, 512));
    }

    #[test]
    fn given_interpolation_enum_when_converting_then_filter_type_matches() {
        assert_eq!(FilterType::from(ResizeInterpolation::Nearest), FilterType::Nearest);
        assert_eq!(FilterType::from(ResizeInterpolation::Bilinear), FilterType::Triangle);
        assert_eq!(FilterType::from(ResizeInterpolation::Bicubic), FilterType::CatmullRom);
        assert_eq!(FilterType::from(ResizeInterpolation::Gaussian), FilterType::Gaussian);
        assert_eq!(FilterType::from(ResizeInterpolation::Lanczos3), FilterType::Lanczos3);
    }
}
