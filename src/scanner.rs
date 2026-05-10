// Video file discovery and directory traversal

use crate::error::ScanError;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Constants for output directory and file naming
pub const BACKDROPS_DIR: &str = "backdrops";
const DONE_MARKER: &str = "done.ext";

pub struct VideoScanner {
    pub root_path: PathBuf,
    pub force: bool,
}

pub struct VideoFile {
    pub path: PathBuf,
    pub parent_dir: PathBuf,
}

pub struct ScanResult {
    pub videos: Vec<VideoFile>,
    pub skipped_dirs: Vec<PathBuf>,
}

impl VideoScanner {
    pub fn new(root_path: PathBuf, force: bool) -> Self {
        Self { root_path, force }
    }

    /// Check if a directory should be skipped.
    /// Only done.ext is the skip signal. If it doesn't exist, the directory is NOT skipped
    /// (the processor will handle counting existing clips and writing done.ext).
    fn should_skip_directory(&self, dir: &Path) -> bool {
        // Skip if it's a backdrops directory
        if dir.file_name().and_then(|n| n.to_str()) == Some(BACKDROPS_DIR) {
            return true;
        }

        // If force mode is enabled, never skip directories
        if self.force {
            return false;
        }

        // Only skip if done.ext marker exists
        let backdrops_dir = dir.join(BACKDROPS_DIR);
        if backdrops_dir.exists() && backdrops_dir.is_dir() {
            let done_marker = backdrops_dir.join(DONE_MARKER);
            if done_marker.exists() && done_marker.is_file() {
                return true;
            }
        }

        false
    }

    /// Check if a directory name matches the movie folder format: "<name> (<year>)"
    /// e.g. "The Matrix (1999)", "Inception (2010)"
    fn is_movie_folder(dir_name: &str) -> bool {
        // Match pattern: anything followed by space, open paren, 4 digits, close paren at end
        if let Some(paren_start) = dir_name.rfind('(') {
            let before_paren = &dir_name[..paren_start];
            let after_paren = &dir_name[paren_start..];
            // Must have content before the paren and a space before it
            if before_paren.ends_with(' ') && !before_paren.trim().is_empty() {
                // Check the year part: "(YYYY)"
                if after_paren.len() == 6
                    && after_paren.ends_with(')')
                    && after_paren[1..5].chars().all(|c| c.is_ascii_digit())
                {
                    return true;
                }
            }
        }
        false
    }

    /// Scan the root directory recursively for video files
    pub fn scan(&self) -> Result<ScanResult, ScanError> {
        let mut videos = Vec::new();
        let mut skipped_dirs = Vec::new();

        for entry in WalkDir::new(&self.root_path).into_iter().filter_entry(|e| {
            // Skip directories that already have backdrops or don't match movie folder format
            if e.file_type().is_dir() {
                let path = e.path();

                let should_skip = self.should_skip_directory(path);
                if should_skip {
                    skipped_dirs.push(path.to_path_buf());
                    return false;
                }

                // For subdirectories (not the root), check if they match movie folder format
                // or are the backdrops directory. Non-movie subdirectories are skipped.
                if path != self.root_path
                    && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
                {
                    // Allow backdrops directories (already handled above)
                    if dir_name == BACKDROPS_DIR {
                        return false; // already filtered by should_skip_directory
                    }
                    // Skip non-movie-format subdirectories
                    if !Self::is_movie_folder(dir_name) {
                        return false;
                    }
                }

                true
            } else {
                true
            }
        }) {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    // Only process files, not directories
                    if !entry.file_type().is_file() {
                        continue;
                    }

                    // Check for video file extensions (.mp4 or .mkv)
                    if let Some(extension) = path.extension()
                        && (extension.eq_ignore_ascii_case("mp4")
                            || extension.eq_ignore_ascii_case("mkv"))
                    {
                        // Skip files named "backdrop.mp4" or "backdrop.mkv" as they're likely output files
                        if let Some(stem) = path.file_stem()
                            && stem.eq_ignore_ascii_case("backdrop")
                        {
                            continue;
                        }

                        // Get the parent directory
                        if let Some(parent) = path.parent() {
                            videos.push(VideoFile {
                                path: path.to_path_buf(),
                                parent_dir: parent.to_path_buf(),
                            });
                        }
                    }
                    // Non-video files are silently skipped (no error)
                }
                Err(err) => {
                    // Handle permission errors gracefully
                    if let Some(io_err) = err.io_error()
                        && io_err.kind() == std::io::ErrorKind::PermissionDenied
                    {
                        // Log warning and continue
                        eprintln!(
                            "Warning: Permission denied: {}",
                            err.path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        );
                        continue;
                    }
                    // For other errors, return an error
                    return Err(ScanError::DirectoryScanFailed(err.to_string()));
                }
            }
        }

        // Sort videos by path for consistent processing order
        videos.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(ScanResult {
            videos,
            skipped_dirs,
        })
    }
}

/// Write a done.ext marker file in the given backdrops directory.
/// The file contains JSON with the current timestamp.
pub fn write_done_marker(backdrops_dir: &Path) -> std::io::Result<()> {
    let now = chrono::Local::now();
    let content = format!(
        "{{\n  \"completed_at\": \"{}\"\n}}\n",
        now.format("%Y-%m-%dT%H:%M:%S%:z")
    );
    std::fs::write(backdrops_dir.join(DONE_MARKER), content)
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
