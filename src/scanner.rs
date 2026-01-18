// Video file discovery and directory traversal

use crate::error::ScanError;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

    /// Check if a directory should be skipped because it already has a backdrop
    fn should_skip_directory(&self, dir: &Path) -> bool {
        dir.join("backdrops").join("backdrop.mp4").exists()
    }

    /// Scan the root directory recursively for video files
    pub fn scan(&self) -> Result<Vec<VideoFile>, ScanError> {
        let mut videos = Vec::new();

        for entry in WalkDir::new(&self.root_path)
            .into_iter()
            .filter_entry(|e| {
                // Skip directories that already have backdrops
                if e.file_type().is_dir() {
                    !self.should_skip_directory(e.path())
                } else {
                    true
                }
            })
        {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    
                    // Only process files, not directories
                    if !entry.file_type().is_file() {
                        continue;
                    }

                    // Check for video file extensions (.mp4 or .mkv)
                    if let Some(extension) = path.extension() {
                        let ext = extension.to_string_lossy().to_lowercase();
                        if ext == "mp4" || ext == "mkv" {
                            // Get the parent directory
                            if let Some(parent) = path.parent() {
                                videos.push(VideoFile {
                                    path: path.to_path_buf(),
                                    parent_dir: parent.to_path_buf(),
                                });
                            }
                        }
                    }
                    // Non-video files are silently skipped (no error)
                }
                Err(err) => {
                    // Handle permission errors gracefully
                    if let Some(io_err) = err.io_error() {
                        if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                            // Log warning and continue
                            eprintln!("Warning: Permission denied: {}", err.path().map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string()));
                            continue;
                        }
                    }
                    // For other errors, return an error
                    return Err(ScanError::DirectoryScanFailed(err.to_string()));
                }
            }
        }

        Ok(videos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use std::io::Write;

    // Helper function to create a temporary directory structure for testing
    fn create_test_directory_structure(
        base_path: &Path,
        depth: usize,
        subdirs_per_level: usize,
    ) -> std::io::Result<Vec<PathBuf>> {
        let mut created_dirs = vec![base_path.to_path_buf()];
        
        if depth == 0 {
            return Ok(created_dirs);
        }

        // Create subdirectories at this level
        for i in 0..subdirs_per_level.min(3) {
            let subdir = base_path.join(format!("subdir_{}", i));
            fs::create_dir_all(&subdir)?;
            
            // Recursively create deeper levels and collect all directories
            let deeper_dirs = create_test_directory_structure(&subdir, depth - 1, subdirs_per_level)?;
            created_dirs.extend(deeper_dirs);
        }

        Ok(created_dirs)
    }

    // Helper function to create video files in directories
    fn create_video_files(dirs: &[PathBuf], files_per_dir: usize) -> std::io::Result<Vec<PathBuf>> {
        let mut video_files = Vec::new();
        
        for dir in dirs {
            for i in 0..files_per_dir {
                let video_path = dir.join(format!("video_{}.mp4", i));
                let mut file = fs::File::create(&video_path)?;
                file.write_all(b"fake video content")?;
                video_files.push(video_path);
            }
        }
        
        Ok(video_files)
    }

    // Feature: video-clip-extractor, Property 1: Recursive Directory Traversal
    proptest! {
        #[test]
        fn test_recursive_directory_traversal(
            depth in 1usize..4,
            subdirs_per_level in 1usize..3,
            files_per_dir in 1usize..3,
        ) {
            // Property: For any directory structure with nested subdirectories,
            // the scanner should discover all subdirectories at any depth level
            // and find all video files within them
            
            // Create a temporary directory for testing
            let temp_dir = std::env::temp_dir().join(format!("video_scanner_test_{}_{}", std::process::id(), rand::random::<u32>()));
            let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
            fs::create_dir_all(&temp_dir).unwrap();
            
            // Create nested directory structure
            let created_dirs = create_test_directory_structure(&temp_dir, depth, subdirs_per_level).unwrap();
            
            // Create video files in each directory
            let expected_videos = create_video_files(&created_dirs, files_per_dir).unwrap();
            
            // Create the scanner
            let scanner = VideoScanner::new(temp_dir.clone());
            
            // Scan for videos
            let result = scanner.scan();
            prop_assert!(result.is_ok(), "Scanner should successfully scan directory structure");
            
            let found_videos = result.unwrap();
            
            // Property 1: All created video files should be discovered
            // The scanner should find exactly as many videos as we created
            prop_assert_eq!(
                found_videos.len(),
                expected_videos.len(),
                "Scanner should find all {} video files across all {} directories (depth={}, subdirs_per_level={}, files_per_dir={})",
                expected_videos.len(),
                created_dirs.len(),
                depth,
                subdirs_per_level,
                files_per_dir
            );
            
            // Property 2: Each found video should be in our expected list
            for video in &found_videos {
                prop_assert!(
                    expected_videos.contains(&video.path),
                    "Found video {:?} should be in expected list",
                    video.path
                );
            }
            
            // Property 3: Each expected video should be found
            for expected in &expected_videos {
                prop_assert!(
                    found_videos.iter().any(|v| v.path == *expected),
                    "Expected video {:?} should be found by scanner",
                    expected
                );
            }
            
            // Property 4: Verify parent_dir is correctly set for each video
            for video in &found_videos {
                prop_assert!(
                    video.path.parent() == Some(video.parent_dir.as_path()),
                    "Video parent_dir should match actual parent of path"
                );
            }
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_dir);
        }
    }

    // Feature: video-clip-extractor, Property 2: Video File Discovery
    proptest! {
        #[test]
        fn test_video_file_discovery(
            num_mp4_files in 0usize..5,
            num_mkv_files in 0usize..5,
            num_other_files in 0usize..5,
        ) {
            // Property: For any directory structure containing files with various extensions,
            // the scanner should include all files with .mp4 or .mkv extensions in the processing list
            // and exclude files with other extensions
            
            // Create a temporary directory for testing
            let temp_dir = std::env::temp_dir().join(format!("video_discovery_test_{}_{}", std::process::id(), rand::random::<u32>()));
            let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
            fs::create_dir_all(&temp_dir).unwrap();
            
            let mut expected_video_files = Vec::new();
            
            // Create .mp4 files (should be discovered)
            for i in 0..num_mp4_files {
                let video_path = temp_dir.join(format!("video_{}.mp4", i));
                let mut file = fs::File::create(&video_path).unwrap();
                file.write_all(b"fake mp4 content").unwrap();
                expected_video_files.push(video_path);
            }
            
            // Create .mkv files (should be discovered)
            for i in 0..num_mkv_files {
                let video_path = temp_dir.join(format!("video_{}.mkv", i));
                let mut file = fs::File::create(&video_path).unwrap();
                file.write_all(b"fake mkv content").unwrap();
                expected_video_files.push(video_path);
            }
            
            // Create files with other extensions (should NOT be discovered)
            let non_video_extensions = vec!["txt", "jpg", "png", "avi", "mov", "doc"];
            for i in 0..num_other_files {
                let ext = non_video_extensions[i % non_video_extensions.len()];
                let file_path = temp_dir.join(format!("file_{}.{}", i, ext));
                let mut file = fs::File::create(&file_path).unwrap();
                file.write_all(b"non-video content").unwrap();
            }
            
            // Create the scanner
            let scanner = VideoScanner::new(temp_dir.clone());
            
            // Scan for videos
            let result = scanner.scan();
            prop_assert!(result.is_ok(), "Scanner should successfully scan directory");
            
            let found_videos = result.unwrap();
            
            // Property 1: Scanner should find exactly the number of .mp4 and .mkv files
            let expected_count = num_mp4_files + num_mkv_files;
            prop_assert_eq!(
                found_videos.len(),
                expected_count,
                "Scanner should find exactly {} video files ({} .mp4 + {} .mkv), but found {}. Non-video files ({}) should be excluded.",
                expected_count,
                num_mp4_files,
                num_mkv_files,
                found_videos.len(),
                num_other_files
            );
            
            // Property 2: All found videos should be .mp4 or .mkv files
            for video in &found_videos {
                let extension = video.path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                prop_assert!(
                    extension == Some("mp4".to_string()) || extension == Some("mkv".to_string()),
                    "Found video {:?} should have .mp4 or .mkv extension, but has {:?}",
                    video.path,
                    extension
                );
            }
            
            // Property 3: All expected video files should be found
            for expected in &expected_video_files {
                prop_assert!(
                    found_videos.iter().any(|v| v.path == *expected),
                    "Expected video {:?} should be found by scanner",
                    expected
                );
            }
            
            // Property 4: Each found video should be in our expected list
            for video in &found_videos {
                prop_assert!(
                    expected_video_files.contains(&video.path),
                    "Found video {:?} should be in expected video list (not a non-video file)",
                    video.path
                );
            }
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_dir);
        }
    }

    #[test]
    fn test_scanner_basic_functionality() {
        // Basic unit test to verify scanner works with a simple structure
        let temp_dir = std::env::temp_dir().join(format!("video_scanner_basic_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a simple video file
        let video_path = temp_dir.join("test.mp4");
        fs::File::create(&video_path).unwrap();
        
        let scanner = VideoScanner::new(temp_dir.clone());
        let result = scanner.scan();
        
        assert!(result.is_ok());
        let videos = result.unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].path, video_path);
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scanner_nested_directories() {
        // Test that scanner finds videos in nested directories
        let temp_dir = std::env::temp_dir().join(format!("video_scanner_nested_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Create nested structure: root/level1/level2/video.mp4
        let level1 = temp_dir.join("level1");
        let level2 = level1.join("level2");
        fs::create_dir_all(&level2).unwrap();
        
        let video1 = temp_dir.join("root_video.mp4");
        let video2 = level1.join("level1_video.mp4");
        let video3 = level2.join("level2_video.mp4");
        
        fs::File::create(&video1).unwrap();
        fs::File::create(&video2).unwrap();
        fs::File::create(&video3).unwrap();
        
        let scanner = VideoScanner::new(temp_dir.clone());
        let result = scanner.scan();
        
        assert!(result.is_ok());
        let videos = result.unwrap();
        assert_eq!(videos.len(), 3, "Should find all 3 videos across all depth levels");
        
        // Verify all videos are found
        let paths: Vec<_> = videos.iter().map(|v| &v.path).collect();
        assert!(paths.contains(&&video1));
        assert!(paths.contains(&&video2));
        assert!(paths.contains(&&video3));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
