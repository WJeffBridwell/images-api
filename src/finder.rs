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

pub fn search_content(image_name: &str) -> Vec<ContentInfo> {
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
        return Vec::new();
    }

    let results = String::from_utf8_lossy(&output.stdout);
    debug!("mdfind raw output: {}", results);
    
    let paths: Vec<&str> = results.split('\n')
        .filter(|s| !s.is_empty())
        .take(20)  // Limit to first 20 results
        .collect();
    
    debug!("Found {} paths after filtering", paths.len());

    info!("Processing {} paths (limited from total found)", paths.len());

    let content_info: Vec<ContentInfo> = paths.iter().filter_map(|path_str| {
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
                debug!("Found user tags section: {}", trimmed);
                in_user_tags = true;
                if trimmed.contains('(') {
                    in_tags_block = true;
                } else if trimmed.contains('=') {
                    // Single line format
                    if let Some(tag) = trimmed.split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string()) {
                        if !tag.is_empty() {
                            debug!("Found single line tag: {}", tag);
                            tags.push(tag);
                        }
                    }
                }
                continue;
            }

            if in_user_tags && in_tags_block {
                if trimmed == ")" {
                    debug!("End of tags block");
                    in_user_tags = false;
                    in_tags_block = false;
                } else {
                    let cleaned = trimmed.trim_matches(',').trim_matches('"').to_string();
                    if !cleaned.is_empty() && cleaned != "(" {
                        debug!("Found tag in block: {}", cleaned);
                        tags.push(cleaned);
                    }
                }
            }
        }

        debug!("Final user tags for {}: {:?}", file_name, tags);

        debug!("Creating ContentInfo for path: {}", path_str);
        debug!("File metadata - size: {}, created: {:?}, accessed: {:?}", 
            metadata.len(), 
            metadata.created().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            metadata.accessed().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
        );
        let content_info = ContentInfo {
            content_name: file_name,
            content_type: extension,
            content_url: path_str.to_string(),
            content_tags: tags,
            content_created: metadata.created().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
            content_viewed: metadata.accessed().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
            content_size: Some(metadata.len() as i64),
        };
        debug!("Created ContentInfo: {:?}", content_info);
        Some(content_info)
    }).collect();

    info!("Returning {} content items", content_info.len());
    content_info
}
