/*! 
 * Images API - Image Processing Module
 * 
 * This module handles all image processing operations including:
 * - Image loading and validation
 * - Format conversion
 * - Resizing and rotation
 * - Metadata extraction
 * 
 * It provides a thread-safe interface for handling image operations
 * with error handling and validation.
 */

use std::path::Path;
use image::{ImageFormat, GenericImageView};
use tokio::fs;
use anyhow::{Result, Context};

/// Image processing error types
#[derive(Debug)]
pub enum ImageError {
    /// File system related errors
    IoError(std::io::Error),
    /// Image processing errors
    ImageError(image::ImageError),
    /// Invalid input parameters
    ValidationError(String),
}

/// Image metadata structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageData {
    /// Raw image bytes
    pub content: Vec<u8>,
    /// Image dimensions (width, height)
    pub dimensions: (u32, u32),
    /// Size of image in bytes
    pub size_bytes: usize,
    /// Image format (jpg, png, etc.)
    pub format: ImageFormat,
}

/// Main image processor struct
pub struct ImageProcessor;

impl ImageProcessor {
    /// Creates a new ImageProcessor instance
    pub fn new() -> Self {
        ImageProcessor
    }

    /// Loads and validates an image file
    /// 
    /// Parameters:
    /// - path: Path to the image file
    /// - include_data: Whether to include raw image data in response
    pub async fn get_image_data(
        &self,
        path: &Path,
        include_data: bool,
    ) -> Result<ImageData, ImageError> {
        let content = fs::read(path)
            .await
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;

        let img = image::load_from_memory(&content)
            .with_context(|| "Failed to load image from memory")?;

        let format = ImageFormat::from_path(path)
            .with_context(|| "Failed to determine image format")?;

        Ok(ImageData {
            dimensions: img.dimensions(),
            format,
            size_bytes: content.len(),
            content,
        })
    }

    /// Resizes an image
    /// 
    /// Parameters:
    /// - path: Path to the image file
    /// - width: Target width
    /// - height: Target height
    pub async fn resize_image(
        &self,
        path: &Path,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, ImageError> {
        let content = fs::read(path)
            .await
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;

        let img = image::load_from_memory(&content)
            .with_context(|| "Failed to load image from memory")?;

        let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);
        
        let format = ImageFormat::from_path(path)
            .with_context(|| "Failed to determine image format")?;

        let mut buffer = Vec::new();
        resized.write_to(&mut std::io::Cursor::new(&mut buffer), format)
            .with_context(|| "Failed to write resized image")?;

        Ok(buffer)
    }

    /// Rotates an image
    /// 
    /// Parameters:
    /// - path: Path to the image file
    /// - angle: Rotation angle in degrees
    pub async fn rotate_image(
        &self,
        path: &Path,
        angle: i32,
    ) -> Result<Vec<u8>, ImageError> {
        let content = fs::read(path)
            .await
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;

        let img = image::load_from_memory(&content)
            .with_context(|| "Failed to load image from memory")?;

        // Normalize degrees to be in [0, 360)
        let normalized_degrees = ((angle % 360) + 360) % 360;
        
        let rotated = match normalized_degrees {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => return Err(anyhow::anyhow!("Only 90-degree rotations are supported")),
        };

        let format = ImageFormat::from_path(path)
            .with_context(|| "Failed to determine image format")?;

        let mut buffer = Vec::new();
        rotated.write_to(&mut std::io::Cursor::new(&mut buffer), format)
            .with_context(|| "Failed to write rotated image")?;

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    /// Creates a test image
    fn create_test_image() -> Vec<u8> {
        // Create a 2x2 black JPEG image
        let img = DynamicImage::new_rgb8(2, 2);
        let mut buffer = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::Jpeg)
            .expect("Failed to create test image");
        buffer
    }

    #[tokio::test]
    async fn test_get_image_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().join("test.jpg");
        
        let test_image = create_test_image();
        std::fs::write(&test_path, &test_image).unwrap();

        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&test_path, false).await;
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.dimensions, (2, 2));
        assert_eq!(data.format, ImageFormat::Jpeg);
        assert!(data.size_bytes > 0);
        assert!(!data.content.is_empty());
    }

    #[tokio::test]
    async fn test_resize_image() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().join("test.jpg");
        
        let test_image = create_test_image();
        std::fs::write(&test_path, &test_image).unwrap();

        let processor = ImageProcessor::new();
        let result = processor.resize_image(&test_path, 4, 4).await;
        
        assert!(result.is_ok());
        let resized_data = result.unwrap();
        assert!(!resized_data.is_empty());
    }

    #[tokio::test]
    async fn test_rotate_image() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().join("test.jpg");
        
        let test_image = create_test_image();
        std::fs::write(&test_path, &test_image).unwrap();

        let processor = ImageProcessor::new();
        let result = processor.rotate_image(&test_path, 90).await;
        
        assert!(result.is_ok());
        let rotated_data = result.unwrap();
        assert!(!rotated_data.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_image() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().join("invalid.jpg");
        
        let invalid_data = b"not an image".to_vec();
        std::fs::write(&test_path, &invalid_data).unwrap();

        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&test_path, false).await;
        
        assert!(result.is_err());
    }
}
