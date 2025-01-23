use std::process::Command;
use std::path::Path;
use serde::{Serialize, Deserialize};
use log::{info, error};

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentInfo {
    pub content_name: String,
    pub content_type: String,
    pub content_url: String,
    pub content_tags: Vec<String>,
    pub content_created: Option<i64>,
    pub content_viewed: Option<i64>,
    pub content_size: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedContentResponse {
    pub items: Vec<ContentInfo>,
    pub total: usize,
    pub page: usize,
    pub total_pages: usize,
    pub page_size: usize,
}

pub fn search_content(image_name: &str, page: usize, page_size: usize) -> PaginatedContentResponse {
    info!("🔍 Search started - name: {}, page: {}, size: {}", image_name, page, page_size);
    
    // Strip extension and create search pattern
    let base_name = image_name.split('.').next().unwrap_or(image_name);
    
    // Search across all volumes using simpler -name approach
    let output = Command::new("mdfind")
        .arg("-0") // Use null byte as separator to handle special characters
        .arg("-name")
        .arg(base_name)
        .output()
        .expect("Failed to execute mdfind command");

    if !output.status.success() {
        error!("mdfind command failed: {:?}", String::from_utf8_lossy(&output.stderr));
        return PaginatedContentResponse {
            items: Vec::new(),
            total: 0,
            page,
            total_pages: 0,
            page_size,
        };
    }

    // Split by null bytes instead of newlines to handle special characters
    let mut all_paths: Vec<String> = output.stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .collect();
    
    info!("📊 Initial paths: {}", all_paths.len());
    
    // Sort and remove duplicates
    all_paths.sort();
    all_paths.dedup();
    
    let total = all_paths.len();
    info!("📊 After dedup: {}", total);
    
    let total_pages = (total + page_size - 1) / page_size;
    let start = (page - 1) * page_size;
    let end = std::cmp::min(start + page_size, total);
    
    info!("📑 Pagination: start={}, end={}, total={}, pages={}, page_size={}", 
          start, end, total, total_pages, page_size);

    // Log the paths we're about to process
    info!("🔍 Processing paths from {} to {}:", start, end);
    all_paths.iter().skip(start).take(end - start).enumerate().for_each(|(i, path)| {
        info!("  [{}] {}", i + start, path);
    });

    let mut filtered_count = 0;
    let mut processed_count = 0;
    let content_info: Vec<ContentInfo> = all_paths
        .iter()
        .skip(start)
        .take(end - start)
        .filter_map(|path_str| {
            processed_count += 1;
            let path = Path::new(path_str);
            if !path.exists() {
                info!("❌ Path does not exist: {}", path_str);
                filtered_count += 1;
                return None;
            }

            let metadata = match path.metadata() {
                Ok(m) => m,
                Err(e) => {
                    info!("❌ Failed to get metadata for {}: {}", path_str, e);
                    filtered_count += 1;
                    return None;
                }
            };

            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => {
                    info!("❌ Failed to get filename for {}", path_str);
                    filtered_count += 1;
                    return None;
                }
            };
            
            let extension = path.extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();

            // Get additional metadata using mdls
            let output = Command::new("mdls")
                .arg(path_str)
                .output()
                .map_err(|e| {
                    info!("❌ Failed to execute mdls for {}: {}", path_str, e);
                    filtered_count += 1;
                    e
                })
                .ok()?;

            let raw_results = String::from_utf8_lossy(&output.stdout).to_string();

            // Extract only user-assigned tags
            let mut tags: Vec<String> = Vec::new();
            let mut in_user_tags = false;
            let mut in_tags_block = false;

            for line in raw_results.lines() {
                let trimmed = line.trim();
                
                if trimmed.starts_with("kMDItemUserTags") {
                    in_user_tags = true;
                    if trimmed.contains('(') {
                        in_tags_block = true;
                    }
                    continue;
                }

                if in_user_tags {
                    if !in_tags_block && trimmed.starts_with('(') {
                        in_tags_block = true;
                        continue;
                    }

                    if in_tags_block {
                        if trimmed == ")" {
                            break;
                        }
                        
                        // Clean up the tag string
                        let tag = trimmed.trim_matches(|c| c == '"' || c == ',').to_string();
                        if !tag.is_empty() {
                            tags.push(tag);
                        }
                    }
                }
            }

            let content_type = if path.is_dir() {
                "folder".to_string()
            } else {
                extension.clone()
            };

            info!("✅ Successfully processed: {}", path_str);
            Some(ContentInfo {
                content_name: file_name,
                content_type,
                content_url: path_str.to_string(),
                content_tags: tags,
                content_created: None,
                content_viewed: None,
                content_size: Some(metadata.len() as i64),
            })
        })
        .collect();

    info!("📊 Final stats: processed={}, returned={}, filtered={}", 
          processed_count, content_info.len(), filtered_count);

    PaginatedContentResponse {
        items: content_info,
        total,
        page,
        total_pages,
        page_size,
    }
}
