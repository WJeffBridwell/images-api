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
}
