//! Decoded images: 8-bit RGBA, row-major, top row first.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, straight (non-premultiplied) alpha.
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Image, String> {
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return Err(format!(
                "image {width}x{height} needs {} bytes, got {}",
                (width as usize) * (height as usize) * 4,
                rgba.len()
            ));
        }
        Ok(Image {
            width,
            height,
            rgba,
        })
    }

    pub fn solid(width: u32, height: u32, color: [u8; 4]) -> Image {
        let rgba = color.repeat((width * height) as usize);
        Image {
            width,
            height,
            rgba,
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&color);
    }

    pub fn from_png_bytes(bytes: &[u8]) -> Result<Image, String> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|e| format!("png: {e}"))?
            .to_rgba8();
        Image::new(img.width(), img.height(), img.into_raw())
    }

    pub fn to_png_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut out,
            &self.rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("png: {e}"))?;
        Ok(out.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let mut img = Image::solid(3, 2, [10, 20, 30, 255]);
        img.set_pixel(2, 1, [1, 2, 3, 4]);
        let back = Image::from_png_bytes(&img.to_png_bytes().unwrap()).unwrap();
        assert_eq!(back, img);
        assert_eq!(back.pixel(2, 1), [1, 2, 3, 4]);
    }

    #[test]
    fn wrong_buffer_size_is_rejected() {
        assert!(Image::new(2, 2, vec![0; 15]).is_err());
        assert!(Image::from_png_bytes(b"not a png").is_err());
    }
}
