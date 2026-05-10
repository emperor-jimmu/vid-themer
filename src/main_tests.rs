use super::*;
use std::path::PathBuf;

#[test]
fn test_directory_not_found_error() {
    let non_existent_path = PathBuf::from("/this/path/definitely/does/not/exist/12345");

    let result = validate_directory(&non_existent_path);

    assert!(result.is_err());
    let err = result.unwrap_err();

    let AppError::DirectoryNotFound(path) = err;
    assert_eq!(path, non_existent_path);
}

#[test]
fn test_path_is_not_directory() {
    let file_path = PathBuf::from("Cargo.toml");

    let result = validate_directory(&file_path);

    assert!(result.is_err());
    let err = result.unwrap_err();

    let AppError::DirectoryNotFound(path) = err;
    assert_eq!(path, file_path);
}

#[test]
fn test_valid_directory() {
    let valid_path = PathBuf::from("src");

    let result = validate_directory(&valid_path);

    assert!(result.is_ok());
}
