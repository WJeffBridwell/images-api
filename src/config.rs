use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub content_directory: String,
}

impl Config {
    pub fn new(content_directory: String) -> Self {
        Self { content_directory }
    }
}
