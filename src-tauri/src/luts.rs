//! The user's looks: `.cube` files kept in the app's data folder, `luts/`.
//! The page reads and parses them; this only stores them.

use std::path::{Path, PathBuf};

/// `<app data>/luts`, created on first use.
pub fn folder(app_data: &Path) -> std::io::Result<PathBuf> {
    let folder = app_data.join("luts");
    std::fs::create_dir_all(&folder)?;
    Ok(folder)
}

/// File names of the stored looks, sorted.
pub fn list(folder: &Path) -> std::io::Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(folder)?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| is_cube(name))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    Ok(names)
}

pub fn save(folder: &Path, name: &str, text: &str) -> std::io::Result<()> {
    std::fs::write(folder.join(checked(name)?), text)
}

pub fn read(folder: &Path, name: &str) -> std::io::Result<String> {
    std::fs::read_to_string(folder.join(checked(name)?))
}

/// A bare `.cube` file name: no folders, so nothing outside `luts/` is reachable.
fn checked(name: &str) -> std::io::Result<&str> {
    let bare = Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
    if bare && is_cube(name) {
        Ok(name)
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("not a .cube file name: {name}")))
    }
}

fn is_cube(name: &str) -> bool {
    name.to_lowercase().ends_with(".cube")
}

#[cfg(test)]
mod tests {
    use super::checked;

    #[test]
    fn only_bare_cube_names_are_accepted() {
        assert!(checked("Ritual Portra 400.cube").is_ok());
        assert!(checked("../escape.cube").is_err());
        assert!(checked("sub/look.cube").is_err());
        assert!(checked("look.txt").is_err());
    }
}
