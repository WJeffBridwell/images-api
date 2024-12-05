use std::path::Path;
use std::sync::Arc;
use std::io::Cursor;
use image::{DynamicImage, ImageFormat, GenericImageView, ImageOutputFormat};
use lru::LruCache;
use tokio::sync::RwLock;
use anyhow::Result;
use base64::Engine;

const THUMBNAIL_SIZE: u32 = 300; // Maximum thumbnail dimension
const CACHE_SIZE: usize = 1000; // Number of items to cache

#[derive(Clone)]
pub struct ImageData {
    pub data_uri: String,
    pub format: ImageFormat,
    pub dimensions: (u32, u32),
    pub size_bytes: usize,
}

pub struct ImageProcessor {
    thumbnail_cache: Arc<RwLock<LruCache<String, ImageData>>>,
    image_cache: Arc<RwLock<LruCache<String, ImageData>>>,
}

impl ImageProcessor {
    pub fn new() -> Self {
        Self {
            thumbnail_cache: Arc::new(RwLock::new(LruCache::new(CACHE_SIZE.try_into().unwrap()))),
            image_cache: Arc::new(RwLock::new(LruCache::new(CACHE_SIZE.try_into().unwrap()))),
        }
    }

    pub async fn get_image_data(&self, path: &Path, thumbnail: bool) -> Result<ImageData> {
        let path_str = path.to_string_lossy().to_string();
        
        // Check cache first
        let cache = if thumbnail {
            &self.thumbnail_cache
        } else {
            &self.image_cache
        };
        
        // Try to get from cache
        {
            let cache_read = cache.read().await;
            if let Some(data) = cache_read.peek(&path_str) {
                return Ok(data.clone());
            }
        }
        
        // Load and process image
        let img = image::open(path)?;
        let format = match path.extension().and_then(|ext| ext.to_str()) {
            Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
            Some("png") => ImageFormat::Png,
            Some("gif") => ImageFormat::Gif,
            _ => ImageFormat::Jpeg, // default
        };
        
        let processed_img = if thumbnail {
            self.create_thumbnail(&img)
        } else {
            img
        };
        
        let data = self.create_image_data(processed_img, format)?;
        
        // Cache the result
        {
            let mut cache_write = cache.write().await;
            cache_write.put(path_str, data.clone());
        }
        
        Ok(data)
    }

    fn create_thumbnail(&self, img: &DynamicImage) -> DynamicImage {
        let (width, height) = img.dimensions();
        
        if width <= THUMBNAIL_SIZE && height <= THUMBNAIL_SIZE {
            return img.clone();
        }
        
        let ratio = width as f32 / height as f32;
        let (new_width, new_height) = if ratio > 1.0 {
            (THUMBNAIL_SIZE, (THUMBNAIL_SIZE as f32 / ratio) as u32)
        } else {
            ((THUMBNAIL_SIZE as f32 * ratio) as u32, THUMBNAIL_SIZE)
        };
        
        img.thumbnail(new_width, new_height)
    }

    fn create_image_data(&self, img: DynamicImage, format: ImageFormat) -> Result<ImageData> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        
        let output_format = match format {
            ImageFormat::Jpeg => ImageOutputFormat::Jpeg(85), // 85% quality
            ImageFormat::Png => ImageOutputFormat::Png,
            ImageFormat::Gif => ImageOutputFormat::Gif,
            _ => ImageOutputFormat::Jpeg(85),
        };
        
        img.write_to(&mut cursor, output_format)?;
        
        let mime_type = match format {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            _ => "image/jpeg",
        };
        
        let data_uri = format!(
            "data:{};base64,{}",
            mime_type,
            base64::engine::general_purpose::STANDARD.encode(&buffer)
        );
        
        Ok(ImageData {
            data_uri,
            format,
            dimensions: img.dimensions(),
            size_bytes: buffer.len(),
        })
    }

    pub async fn clear_caches(&self) {
        self.thumbnail_cache.write().await.clear();
        self.image_cache.write().await.clear();
    }
}
