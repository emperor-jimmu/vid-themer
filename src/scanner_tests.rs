use super::*;
use proptest::prelude::*;
use std::fs;
use std::io::Write;
use std::path::Path;

fn create_test_directory_structure(
    base_path: &Path,
    depth: usize,
    subdirs_per_level: usize,
) -> std::io::Result<Vec<PathBuf>> {
    let mut created_dirs = vec![base_path.to_path_buf()];

    if depth == 0 {
        return Ok(created_dirs);
    }

    for i in 0..subdirs_per_level.min(3) {
        let subdir = base_path.join(format!("Movie {} (200{})", i, i));
        fs::create_dir_all(&subdir)?;

        let deeper_dirs = create_test_directory_structure(&subdir, depth - 1, subdirs_per_level)?;
        created_dirs.extend(deeper_dirs);
    }

    Ok(created_dirs)
}

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

proptest! {
    #[test]
    fn test_recursive_directory_traversal(
        depth in 1usize..4,
        subdirs_per_level in 1usize..3,
        files_per_dir in 1usize..3,
    ) {
        let temp_dir = std::env::temp_dir().join(format!(
            "video_scanner_test_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let created_dirs = create_test_directory_structure(&temp_dir, depth, subdirs_per_level).unwrap();
        let expected_videos = create_video_files(&created_dirs, files_per_dir).unwrap();

        let scanner = VideoScanner::new(temp_dir.clone(), false);
        let result = scanner.scan();
        prop_assert!(result.is_ok());

        let found_videos = result.unwrap().videos;
        prop_assert_eq!(found_videos.len(), expected_videos.len());

        for video in &found_videos {
            prop_assert!(expected_videos.contains(&video.path));
            prop_assert_eq!(video.path.parent(), Some(video.parent_dir.as_path()));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

#[test]
fn test_scanner_basic_functionality() {
    let temp_dir = std::env::temp_dir().join(format!("video_scanner_basic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let video_path = temp_dir.join("test.mp4");
    fs::File::create(&video_path).unwrap();

    let scanner = VideoScanner::new(temp_dir.clone(), false);
    let result = scanner.scan().unwrap();
    assert_eq!(result.videos.len(), 1);
    assert_eq!(result.videos[0].path, video_path);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scanner_sorts_videos_alphabetically() {
    let temp_dir = std::env::temp_dir().join(format!("video_scanner_sort_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let video_c = temp_dir.join("c_video.mp4");
    let video_a = temp_dir.join("a_video.mp4");
    let video_b = temp_dir.join("b_video.mp4");

    fs::File::create(&video_c).unwrap();
    fs::File::create(&video_a).unwrap();
    fs::File::create(&video_b).unwrap();

    let videos = VideoScanner::new(temp_dir.clone(), false)
        .scan()
        .unwrap()
        .videos;
    assert_eq!(videos.len(), 3);
    assert_eq!(videos[0].path, video_a);
    assert_eq!(videos[1].path, video_b);
    assert_eq!(videos[2].path, video_c);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_movie_folder_valid_and_invalid() {
    for valid in [
        "The Matrix (1999)",
        "Inception (2010)",
        "A (0000)",
        "Movie 2 (2024)",
        "Some Movie - Extended (2015)",
    ] {
        assert!(VideoScanner::is_movie_folder(valid));
    }

    for invalid in [
        "backdrops",
        "Extras",
        "Featurettes",
        "subdir_0",
        "(2020)",
        "Movie(2020)",
        "Movie (20)",
        "Movie (20200)",
        "Movie (abcd)",
    ] {
        assert!(!VideoScanner::is_movie_folder(invalid));
    }
}

#[test]
fn test_non_movie_subdirectories_are_skipped() {
    let temp_dir = std::env::temp_dir().join(format!(
        "non_movie_subdir_test_{}_{}",
        std::process::id(),
        rand::random::<u32>()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let movie_dir = temp_dir.join("The Matrix (1999)");
    fs::create_dir_all(&movie_dir).unwrap();
    let movie_video = movie_dir.join("movie.mp4");
    fs::File::create(&movie_video).unwrap();

    let extras_dir = movie_dir.join("Extras");
    fs::create_dir_all(&extras_dir).unwrap();
    fs::File::create(extras_dir.join("extra.mp4")).unwrap();

    let random_dir = temp_dir.join("some_random_folder");
    fs::create_dir_all(&random_dir).unwrap();
    fs::File::create(random_dir.join("random.mp4")).unwrap();

    let result = VideoScanner::new(temp_dir.clone(), false).scan().unwrap();
    assert_eq!(result.videos.len(), 1);
    assert_eq!(result.videos[0].path, movie_video);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_done_marker_skips_directory_and_force_mode_ignores_it() {
    let temp_dir = std::env::temp_dir().join(format!(
        "done_marker_test_{}_{}",
        std::process::id(),
        rand::random::<u32>()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let done_movie_dir = temp_dir.join("Done Movie (2020)");
    fs::create_dir_all(&done_movie_dir).unwrap();
    let done_video = done_movie_dir.join("movie.mp4");
    fs::File::create(&done_video).unwrap();
    let backdrops_dir = done_movie_dir.join("backdrops");
    fs::create_dir_all(&backdrops_dir).unwrap();
    write_done_marker(&backdrops_dir).unwrap();

    let undone_movie_dir = temp_dir.join("Undone Movie (2021)");
    fs::create_dir_all(&undone_movie_dir).unwrap();
    let undone_video = undone_movie_dir.join("movie.mp4");
    fs::File::create(&undone_video).unwrap();

    let skipped = VideoScanner::new(temp_dir.clone(), false).scan().unwrap();
    assert_eq!(skipped.videos.len(), 1);
    assert_eq!(skipped.videos[0].path, undone_video);

    let forced = VideoScanner::new(temp_dir.clone(), true).scan().unwrap();
    assert_eq!(forced.videos.len(), 2);
    assert!(forced.videos.iter().any(|v| v.path == done_video));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_done_marker_json_content() {
    let temp_dir = std::env::temp_dir().join(format!(
        "done_marker_json_test_{}_{}",
        std::process::id(),
        rand::random::<u32>()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    write_done_marker(&temp_dir).unwrap();

    let content = fs::read_to_string(temp_dir.join("done.ext")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("completed_at").is_some());
    assert!(parsed["completed_at"].is_string());

    let _ = fs::remove_dir_all(&temp_dir);
}
