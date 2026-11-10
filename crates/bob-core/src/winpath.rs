//! Windows extended-length paths. Unicode is already wide; paths past
//! MAX_PATH need the `\\?\` prefix so ffmpeg and fpcalc can open them.

use std::ffi::OsString;
use std::path::Path;

pub fn extended_path(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        let text = String::from_utf16_lossy(&wide);
        if path.is_absolute() && !text.starts_with(r"\\?\") && text.chars().count() >= 240 {
            return OsString::from(format!(r"\\?\{text}"));
        }
        return path.as_os_str().to_os_string();
    }
    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_unicode_path_is_left_alone() {
        let path = Path::new("C:/músicas/ação.opus");
        let extended = extended_path(path);
        assert_eq!(extended, path.as_os_str());
    }
}
