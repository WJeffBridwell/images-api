use std::process::Command;
use std::path::Path;
use serde::{Serialize, Deserialize};
use log::{info, error, debug};

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
    debug!("Starting search_content for image_name: {}", image_name);
    
    // Strip extension from image name for search
    let base_name = image_name.split('.').next().unwrap_or(image_name);
    let args = vec!["-name", base_name];
    debug!("Running mdfind command: mdfind {}", args.join(" "));

    let output = Command::new("mdfind")
        .args(&args)
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

    let results = String::from_utf8_lossy(&output.stdout);
    debug!("mdfind raw output: {}", results);
    
    let all_paths: Vec<&str> = results.split('\n')
        .filter(|s| !s.is_empty())
        .collect();
    
    let total = all_paths.len();
    let total_pages = (total + page_size - 1) / page_size;
    
    // Calculate start and end indices for the current page
    let start = (page - 1) * page_size;
    let end = start + page_size;
    
    debug!("Found {} total paths, processing page {} (items {}-{})", total, page, start, end);

    let content_info: Vec<ContentInfo> = all_paths
        .iter()
        .skip(start)
        .take(page_size)
        .filter_map(|path_str| {
            let path = Path::new(path_str);
            if !path.exists() {
                error!("Path does not exist: {}", path_str);
                return None;
            }

            debug!("Processing path: {}", path_str);

            let metadata = match path.metadata() {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to get metadata for {}: {}", path_str, e);
                    return None;
                }
            };

            debug!("Got metadata for: {}", path_str);

            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => {
                    error!("Failed to get filename for {}", path_str);
                    return None;
                }
            };
            
            let extension = path.extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();

            debug!("Processing file: {} with extension: {}", file_name, extension);

            // Get additional metadata using mdls
            let output = Command::new("mdls")
                .arg(path_str)
                .output()
                .map_err(|e| {
                    error!("Failed to execute mdls for {}: {}", path_str, e);
                    e
                })
                .ok()?;

            let raw_results = String::from_utf8_lossy(&output.stdout).to_string();
            debug!("Full mdls output for {}: {}", path_str, raw_results);

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

            Some(ContentInfo {
                content_name: file_name,
                content_type,
                content_url: path_str.to_string(),
                content_tags: tags,
                content_created: None, // TODO: Add creation time
                content_viewed: None,  // TODO: Add last viewed time
                content_size: Some(metadata.len() as i64),
            })
        })
        .collect();

    PaginatedContentResponse {
        items: content_info,
        total,
        page,
        total_pages,
        page_size,
    }
}
