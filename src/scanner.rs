// Video file discovery and directory traversal

use std::path::PathBuf;

pub struct VideoScanner {
    pub root_path: PathBuf,
}

pub struct VideoFile {
    pub path: PathBuf,
    pub parent_dir: PathBuf,
}

impl VideoScanner {
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }
}
