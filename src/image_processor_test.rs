#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use image::{ImageBuffer, Rgb};

    fn create_test_image(dir: &TempDir, filename: &str) -> PathBuf {
        let path = dir.path().join(filename);
        let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(100, 100);
        img.save(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn test_get_image_data() {
        let temp_dir = TempDir::new().unwrap();
        let test_image_path = create_test_image(&temp_dir, "test.jpg");
        
        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&test_image_path).await;
        
        assert!(result.is_ok());
        let image_data = result.unwrap();
        assert!(image_data.data_uri.starts_with("data:image/jpeg;base64,"));
    }

    #[tokio::test]
    async fn test_get_image_dimensions() {
        let temp_dir = TempDir::new().unwrap();
        let test_image_path = create_test_image(&temp_dir, "test.jpg");
        
        let processor = ImageProcessor::new();
        let result = processor.get_image_dimensions(&test_image_path).await;
        
        assert!(result.is_ok());
        let dimensions = result.unwrap();
        assert_eq!(dimensions, (100, 100));
    }

    #[tokio::test]
    async fn test_create_thumbnail() {
        let temp_dir = TempDir::new().unwrap();
        let test_image_path = create_test_image(&temp_dir, "test.jpg");
        
        let processor = ImageProcessor::new();
        let result = processor.create_thumbnail(&test_image_path, 50, 50).await;
        
        assert!(result.is_ok());
        let thumbnail_data = result.unwrap();
        assert!(thumbnail_data.data_uri.starts_with("data:image/jpeg;base64,"));
    }

    #[tokio::test]
    async fn test_invalid_image() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_path = temp_dir.path().join("nonexistent.jpg");
        
        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&invalid_path).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cache_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let test_image_path = create_test_image(&temp_dir, "test.jpg");
        
        let processor = ImageProcessor::new();
        
        // First request should populate cache
        let result1 = processor.get_image_data(&test_image_path).await;
        assert!(result1.is_ok());
        
        // Second request should use cache
        let result2 = processor.get_image_data(&test_image_path).await;
        assert!(result2.is_ok());
        
        // Results should be identical
        assert_eq!(result1.unwrap().data_uri, result2.unwrap().data_uri);
    }

    #[test]
    fn test_invalid_image_path() {
        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&PathBuf::from("/nonexistent/image.jpg"));
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_image_data() {
        let processor = ImageProcessor::new();
        let invalid_data = vec![0, 1, 2, 3]; // Invalid image data
        let result = processor.process_image_data(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_invalid_dimensions() {
        let processor = ImageProcessor::new();
        let result = processor.resize_image(&vec![0, 1, 2, 3], 0, 0); // Invalid dimensions
        assert!(result.is_err());
    }

    #[test]
    fn test_rotate_invalid_image() {
        let processor = ImageProcessor::new();
        let result = processor.rotate_image(&vec![0, 1, 2, 3], 90.0); // Invalid image data
        assert!(result.is_err());
    }

    #[test]
    fn test_process_empty_image_data() {
        let processor = ImageProcessor::new();
        let result = processor.process_image_data(&vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_negative_dimensions() {
        let processor = ImageProcessor::new();
        let result = processor.resize_image(&vec![0, 1, 2, 3], -100, -100);
        assert!(result.is_err());
    }

    #[test]
    fn test_rotate_invalid_angle() {
        let processor = ImageProcessor::new();
        let result = processor.rotate_image(&vec![0, 1, 2, 3], 360.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_image_data_directory() {
        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&PathBuf::from("/tmp")); // Try to read a directory
        assert!(result.is_err());
    }

    #[test]
    fn test_get_image_data_no_permissions() {
        let processor = ImageProcessor::new();
        let result = processor.get_image_data(&PathBuf::from("/root/test.jpg")); // Try to read from restricted path
        assert!(result.is_err());
    }

    #[test]
    fn test_process_image_data_corrupt() {
        let processor = ImageProcessor::new();
        let corrupt_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // Invalid JPEG header
        let result = processor.process_image_data(&corrupt_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_image_zero_dimensions() {
        let processor = ImageProcessor::new();
        let result = processor.resize_image(&vec![0, 1, 2, 3], 0, 100); // Zero width
        assert!(result.is_err());

        let result = processor.resize_image(&vec![0, 1, 2, 3], 100, 0); // Zero height
        assert!(result.is_err());
    }

    #[test]
    fn test_rotate_image_invalid_angles() {
        let processor = ImageProcessor::new();
        let test_data = vec![0, 1, 2, 3];
        
        let result = processor.rotate_image(&test_data, -90.0); // Negative angle
        assert!(result.is_err());
        
        let result = processor.rotate_image(&test_data, 450.0); // Angle > 360
        assert!(result.is_err());
    }

    #[test]
    fn test_process_image_data_large_size() {
        let processor = ImageProcessor::new();
        let large_data = vec![0; 100 * 1024 * 1024]; // 100MB of data
        let result = processor.process_image_data(&large_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_image_extreme_dimensions() {
        let processor = ImageProcessor::new();
        let result = processor.resize_image(&vec![0, 1, 2, 3], 100000, 100000); // Very large dimensions
        assert!(result.is_err());
    }
}
