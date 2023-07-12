use std::path::Path;

pub fn should_ignore_file(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    
    if !path.is_file() {
        return true;
    }
    
    let name = std::env::current_exe().unwrap();
    
    if path == name {
        return true;
    }
    
    let name = path.file_name().unwrap();
    
    if name == ".DS_Store" {
        return true;
    }
    
    if name == ".localized" {
        return true;
    }
    
    if name == "Thumbs.db" {
        return true;
    }
    
    if name == ".gitignore" {
        return true;
    }
    
    if name == ".svn" {
        return true;
    }
    
    if name == ".idea" {
        return true;
    }
    return false;
}
    
pub fn should_ignore_files(path: &Path, second_path: &Path) -> bool {
    if should_ignore_file(path) {
        return true;
    }
    if should_ignore_file(second_path) {
        return true;
    }
    
    if path.eq(second_path) {
        return true;
    }
    
    return false;
}      