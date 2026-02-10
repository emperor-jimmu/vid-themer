// Video file discovery and directory traversal

use crate::error::ScanError;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Constants for output directory and file naming
const BACKDROPS_DIR: &str = "backdrops";
const BACKDROP_FILE: &str = "backdrop.mp4";

pub struct VideoScanner {
    pub root_path: PathBuf,
    pub requested_clip_count: u8,
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
    pub fn new(root_path: PathBuf, requested_clip_count: u8) -> Self {
        Self {
            root_path,
            requested_clip_count,
        }
    }

    /// Check if a directory should be skipped
    fn should_skip_directory(&self, dir: &Path) -> bool {
        // Skip if it's a backdrops directory
        if dir.file_name().and_then(|n| n.to_str()) == Some(BACKDROPS_DIR) {
            return true;
        }

        // Check if it already has enough valid backdrop files (non-zero size)
        let backdrops_dir = dir.join(BACKDROPS_DIR);
        if backdrops_dir.exists() && backdrops_dir.is_dir() {
            let existing_count = self.count_valid_backdrop_files(&backdrops_dir);
            // Skip only if we have enough or more clips than requested
            return existing_count >= self.requested_clip_count;
        }

        false
    }

    /// Count the number of valid (non-zero size) backdrop files in sequential order
    /// Returns the count of backdrop1.mp4, backdrop2.mp4, etc. that exist and are valid
    fn count_valid_backdrop_files(&self, backdrops_dir: &Path) -> u8 {
        let mut count = 0u8;
        
        // Check for backdrop files in sequential order (backdrop1.mp4, backdrop2.mp4, etc.)
        for i in 1..=4 {
            let backdrop_path = backdrops_dir.join(format!("backdrop{}.mp4", i));
            
            if let Ok(metadata) = std::fs::metadata(&backdrop_path) {
                if metadata.is_file() && metadata.len() > 0 {
                    count += 1;
                } else {
                    // Stop counting if we hit a zero-byte or invalid file
                    break;
                }
            } else {
                // Stop counting if the file doesn't exist
                break;
            }
        }
        
        count
    }

    /// Scan the root directory recursively for video files
    pub fn scan(&self) -> Result<ScanResult, ScanError> {
        let mut videos = Vec::new();
        let mut skipped_dirs = Vec::new();

        for entry in WalkDir::new(&self.root_path).into_iter().filter_entry(|e| {
            // Skip directories that already have backdrops
            if e.file_type().is_dir() {
                let should_skip = self.should_skip_directory(e.path());
                if should_skip {
                    // Track skipped directories (including root if it has a backdrop)
                    skipped_dirs.push(e.path().to_path_buf());
                }
                !should_skip
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
                    if let Some(extension) = path.extension() {
                        let ext = extension.to_string_lossy().to_lowercase();
                        if ext == "mp4" || ext == "mkv" {
                            // Skip files named "backdrop.mp4" or "backdrop.mkv" as they're likely output files
                            if let Some(filename) = path.file_name() {
                                let filename_str = filename.to_string_lossy().to_lowercase();
                                if filename_str == BACKDROP_FILE || filename_str == "backdrop.mkv" {
                                    continue;
                                }
                            }

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

        Ok(ScanResult {
            videos,
            skipped_dirs,
        })
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
            let deeper_dirs =
                create_test_directory_structure(&subdir, depth - 1, subdirs_per_level)?;
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

            // Create the scanner (with default clip_count of 1 for testing)
            let scanner = VideoScanner::new(temp_dir.clone(), 1);

            // Scan for videos
            let result = scanner.scan();
            prop_assert!(result.is_ok(), "Scanner should successfully scan directory structure");

            let scan_result = result.unwrap();
            let found_videos = scan_result.videos;

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

            // Create the scanner (with default clip_count of 1 for testing)
            let scanner = VideoScanner::new(temp_dir.clone(), 1);

            // Scan for videos
            let result = scanner.scan();
            prop_assert!(result.is_ok(), "Scanner should successfully scan directory");

            let scan_result = result.unwrap();
            let found_videos = scan_result.videos;

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

    // Feature: video-clip-extractor, Property 4: Non-Video File Filtering
    proptest! {
        #[test]
        fn test_non_video_file_filtering(
            num_video_files in 0usize..5,
            num_non_video_files in 1usize..10,
        ) {
            // Property: For any directory structure containing files with non-video extensions,
            // the scanner should exclude those files without raising errors

            // Create a temporary directory for testing
            let temp_dir = std::env::temp_dir().join(format!("non_video_filter_test_{}_{}", std::process::id(), rand::random::<u32>()));
            let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
            fs::create_dir_all(&temp_dir).unwrap();

            let mut expected_video_files = Vec::new();

            // Create video files (should be discovered)
            for i in 0..num_video_files {
                let ext = if i % 2 == 0 { "mp4" } else { "mkv" };
                let video_path = temp_dir.join(format!("video_{}.{}", i, ext));
                let mut file = fs::File::create(&video_path).unwrap();
                file.write_all(b"fake video content").unwrap();
                expected_video_files.push(video_path);
            }

            // Create non-video files with various extensions (should be silently skipped)
            let non_video_extensions = vec![
                "txt", "jpg", "png", "gif", "bmp", "svg",  // Images and text
                "avi", "mov", "wmv", "flv", "webm",        // Other video formats (not supported)
                "mp3", "wav", "flac", "aac",               // Audio files
                "doc", "pdf", "zip", "tar", "gz",          // Documents and archives
                "exe", "dll", "so", "dylib",               // Executables and libraries
                "json", "xml", "yaml", "toml", "ini",      // Config files
                "rs", "py", "js", "java", "cpp", "h",      // Source code
            ];

            for i in 0..num_non_video_files {
                let ext = non_video_extensions[i % non_video_extensions.len()];
                let file_path = temp_dir.join(format!("file_{}.{}", i, ext));
                let mut file = fs::File::create(&file_path).unwrap();
                file.write_all(format!("content for .{} file", ext).as_bytes()).unwrap();
            }

            // Create the scanner (with default clip_count of 1 for testing)
            let scanner = VideoScanner::new(temp_dir.clone(), 1);

            // Scan for videos - this should NOT produce errors despite non-video files
            let result = scanner.scan();

            // Property 1: Scanner should succeed without errors even with non-video files present
            prop_assert!(
                result.is_ok(),
                "Scanner should successfully scan directory with {} non-video files without errors",
                num_non_video_files
            );

            let scan_result = result.unwrap();
            let found_videos = scan_result.videos;

            // Property 2: Scanner should find only video files, excluding all non-video files
            prop_assert_eq!(
                found_videos.len(),
                num_video_files,
                "Scanner should find exactly {} video files, ignoring {} non-video files",
                num_video_files,
                num_non_video_files
            );

            // Property 3: All found files should be video files (.mp4 or .mkv)
            for video in &found_videos {
                let extension = video.path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                prop_assert!(
                    extension == Some("mp4".to_string()) || extension == Some("mkv".to_string()),
                    "Found file {:?} should be a video file (.mp4 or .mkv), not {:?}",
                    video.path,
                    extension
                );
            }

            // Property 4: All expected video files should be found
            for expected in &expected_video_files {
                prop_assert!(
                    found_videos.iter().any(|v| v.path == *expected),
                    "Expected video {:?} should be found by scanner",
                    expected
                );
            }

            // Property 5: No non-video files should be in the results
            for video in &found_videos {
                let extension = video.path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());

                if let Some(ext) = extension {
                    prop_assert!(
                        !non_video_extensions.contains(&ext.as_str()) || ext == "mp4" || ext == "mkv",
                        "Non-video file with extension .{} should not be in results",
                        ext
                    );
                }
            }

            // Clean up
            let _ = fs::remove_dir_all(&temp_dir);
        }
    }

    #[test]
    fn test_scanner_basic_functionality() {
        // Basic unit test to verify scanner works with a simple structure
        let temp_dir =
            std::env::temp_dir().join(format!("video_scanner_basic_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a simple video file
        let video_path = temp_dir.join("test.mp4");
        fs::File::create(&video_path).unwrap();

        let scanner = VideoScanner::new(temp_dir.clone(), 1);
        let result = scanner.scan();

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        let videos = scan_result.videos;
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].path, video_path);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scanner_nested_directories() {
        // Test that scanner finds videos in nested directories
        let temp_dir =
            std::env::temp_dir().join(format!("video_scanner_nested_{}", std::process::id()));
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

        let scanner = VideoScanner::new(temp_dir.clone(), 1);
        let result = scanner.scan();

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        let videos = scan_result.videos;
        assert_eq!(
            videos.len(),
            3,
            "Should find all 3 videos across all depth levels"
        );

        // Verify all videos are found
        let paths: Vec<_> = videos.iter().map(|v| &v.path).collect();
        assert!(paths.contains(&&video1));
        assert!(paths.contains(&&video2));
        assert!(paths.contains(&&video3));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // Feature: video-clip-extractor, Property 3: Skip Directories with Existing Clips
    proptest! {
        #[test]
        fn test_skip_directories_with_existing_clips(
            num_dirs_with_clips in 0usize..5,
            num_dirs_without_clips in 1usize..5,
            videos_per_dir in 1usize..3,
        ) {
            // Property: For any directory containing a backdrops/backdrop1.mp4 file,
            // the scanner should exclude all video files in that directory from the processing list
            // when clip_count is 1 (since it already has 1 clip)

            // Create a temporary directory for testing
            let temp_dir = std::env::temp_dir().join(format!(
                "skip_dirs_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
            fs::create_dir_all(&temp_dir).unwrap();

            let mut expected_videos = Vec::new();
            let mut skipped_videos = Vec::new();

            // Create directories WITH existing backdrops/backdrop1.mp4 (should be skipped with clip_count=1)
            for i in 0..num_dirs_with_clips {
                let dir = temp_dir.join(format!("with_clip_{}", i));
                fs::create_dir_all(&dir).unwrap();

                // Create the backdrops/backdrop1.mp4 file (new naming convention)
                let backdrops_dir = dir.join("backdrops");
                fs::create_dir_all(&backdrops_dir).unwrap();
                let backdrop_file = backdrops_dir.join("backdrop1.mp4");
                let mut file = fs::File::create(&backdrop_file).unwrap();
                file.write_all(b"existing backdrop content").unwrap();

                // Create video files in this directory (should be skipped)
                for j in 0..videos_per_dir {
                    let video_path = dir.join(format!("video_{}.mp4", j));
                    let mut file = fs::File::create(&video_path).unwrap();
                    file.write_all(b"video content").unwrap();
                    skipped_videos.push(video_path);
                }
            }

            // Create directories WITHOUT existing backdrops (should be scanned)
            for i in 0..num_dirs_without_clips {
                let dir = temp_dir.join(format!("without_clip_{}", i));
                fs::create_dir_all(&dir).unwrap();

                // Create video files in this directory (should be found)
                for j in 0..videos_per_dir {
                    let video_path = dir.join(format!("video_{}.mp4", j));
                    let mut file = fs::File::create(&video_path).unwrap();
                    file.write_all(b"video content").unwrap();
                    expected_videos.push(video_path);
                }
            }

            // Create the scanner (with default clip_count of 1 for testing)
            let scanner = VideoScanner::new(temp_dir.clone(), 1);

            // Scan for videos
            let result = scanner.scan();
            prop_assert!(
                result.is_ok(),
                "Scanner should successfully scan directory structure"
            );

            let scan_result = result.unwrap();
            let found_videos = scan_result.videos;

            // Property 1: Scanner should find only videos from directories WITHOUT existing clips
            prop_assert_eq!(
                found_videos.len(),
                expected_videos.len(),
                "Scanner should find exactly {} videos from {} directories without clips, \
                 ignoring {} videos from {} directories with existing clips",
                expected_videos.len(),
                num_dirs_without_clips,
                skipped_videos.len(),
                num_dirs_with_clips
            );

            // Property 2: All found videos should be from directories without existing clips
            for video in &found_videos {
                prop_assert!(
                    expected_videos.contains(&video.path),
                    "Found video {:?} should be from a directory without existing clips",
                    video.path
                );
            }

            // Property 3: No videos from directories with existing clips should be found
            for video in &found_videos {
                prop_assert!(
                    !skipped_videos.contains(&video.path),
                    "Found video {:?} should NOT be from a directory with existing clips",
                    video.path
                );
            }

            // Property 4: All expected videos should be found
            for expected in &expected_videos {
                prop_assert!(
                    found_videos.iter().any(|v| v.path == *expected),
                    "Expected video {:?} from directory without clips should be found",
                    expected
                );
            }

            // Property 5: Verify that directories with backdrops/backdrop1.mp4 are actually skipped
            // by checking that none of their videos appear in results
            for skipped in &skipped_videos {
                prop_assert!(
                    !found_videos.iter().any(|v| v.path == *skipped),
                    "Skipped video {:?} from directory with existing clip should NOT be found",
                    skipped
                );
            }

            // Clean up
            let _ = fs::remove_dir_all(&temp_dir);
        }
    }
}
