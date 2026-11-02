use std::cmp::Ordering;
use std::path::Path;
use thiserror::Error;

/// Identify an executable by its lexical absolute Windows path.
#[derive(Clone, Debug)]
pub struct ExecutablePath {
    text: String,
    #[cfg(windows)]
    wide: Vec<u16>,
    #[cfg(windows)]
    length: i32,
}

/// Report an unsupported or malformed absolute executable path.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid absolute executable path")]
pub struct InvalidExecutablePath;

impl ExecutablePath {
    /// Normalize an absolute drive or UNC executable path without file access.
    ///
    /// # Errors
    ///
    /// Reject unsupported roots, invalid components, traversal above the root,
    /// paths without an executable filename, and unrepresentable UTF-16
    /// lengths.
    pub fn new(input: &str) -> Result<Self, InvalidExecutablePath> {
        let separated = input.replace('/', "\\");
        let unc;
        let ordinary = if let Some(extended) = separated.strip_prefix("\\\\?\\") {
            if extended
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"))
            {
                unc = format!("\\\\{}", extended.get(4..).ok_or(InvalidExecutablePath)?);
                &unc
            } else if matches!(extended.as_bytes(), [drive, b':', b'\\', ..] if drive.is_ascii_alphabetic())
            {
                extended
            } else {
                return Err(InvalidExecutablePath);
            }
        } else {
            &separated
        };
        let (root, rest) = root(ordinary)?;
        if rest.ends_with('\\') || matches!(rest.rsplit('\\').next(), Some("." | ".." | "")) {
            return Err(InvalidExecutablePath);
        }
        let mut parts = Vec::new();
        for part in rest.split('\\').filter(|part| !part.is_empty()) {
            match part {
                "." => {}
                ".." => {
                    parts.pop().ok_or(InvalidExecutablePath)?;
                }
                _ if !valid_component(part) => return Err(InvalidExecutablePath),
                _ => parts.push(part),
            }
        }
        let name = parts.last().ok_or(InvalidExecutablePath)?;
        crate::normalize_executable(name).map_err(|_error| InvalidExecutablePath)?;
        let text = format!("{root}{}", parts.join("\\"));
        #[cfg(windows)]
        let length =
            i32::try_from(text.encode_utf16().count()).map_err(|_error| InvalidExecutablePath)?;
        #[cfg(windows)]
        let wide = text.encode_utf16().collect();
        Ok(Self {
            text,
            #[cfg(windows)]
            wide,
            #[cfg(windows)]
            length,
        })
    }

    /// Return the normalized representative spelling.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Return the executable filename.
    pub fn file_name(&self) -> &str {
        self.text.rsplit('\\').next().unwrap_or(&self.text)
    }

    /// Return the containing folder.
    pub fn folder(&self) -> &str {
        self.text.rsplit_once('\\').map_or("", |(folder, _)| folder)
    }

    /// Return the normalized path for file operations.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.text)
    }
}

fn root(path: &str) -> Result<(String, &str), InvalidExecutablePath> {
    if let Some(unc) = path.strip_prefix("\\\\") {
        let unc = unc.trim_start_matches('\\');
        let mut parts = unc.split('\\').filter(|part| !part.is_empty());
        let server = parts.next().ok_or(InvalidExecutablePath)?;
        let share = parts.next().ok_or(InvalidExecutablePath)?;
        if !valid_component(server) || !valid_component(share) {
            return Err(InvalidExecutablePath);
        }
        let rest = unc
            .strip_prefix(server)
            .ok_or(InvalidExecutablePath)?
            .trim_start_matches('\\')
            .strip_prefix(share)
            .ok_or(InvalidExecutablePath)?;
        Ok((format!("\\\\{server}\\{share}\\"), rest))
    } else if let [drive, b':', b'\\', ..] = path.as_bytes()
        && drive.is_ascii_alphabetic()
    {
        Ok((
            format!("{}:\\", char::from(*drive)),
            path.get(3..).ok_or(InvalidExecutablePath)?,
        ))
    } else {
        Err(InvalidExecutablePath)
    }
}

fn valid_component(part: &str) -> bool {
    if part.is_empty()
        || part.ends_with([' ', '.'])
        || part
            .chars()
            .any(|c| c <= '\u{1f}' || "<>:\"|?*".contains(c))
    {
        return false;
    }
    let stem = part.split('.').next().unwrap_or_default().to_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
}

impl PartialEq for ExecutablePath {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for ExecutablePath {}
impl PartialOrd for ExecutablePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ExecutablePath {
    fn cmp(&self, other: &Self) -> Ordering {
        #[cfg(windows)]
        {
            crate::platform::executable_path::compare(
                &self.wide,
                self.length,
                &other.wide,
                other.length,
            )
        }
        #[cfg(not(windows))]
        {
            self.text
                .chars()
                .flat_map(char::to_uppercase)
                .cmp(other.text.chars().flat_map(char::to_uppercase))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(r"C:\Apps\A\..\browser.exe", r"C:\Apps\browser.exe")]
    #[test_case(r"\\?\C:\Apps\browser.exe", r"C:/apps/browser.EXE")]
    #[test_case(
        r"\\server\share\A\..\browser.exe",
        r"\\?\UNC\SERVER\share\browser.exe"
    )]
    #[test_case(r"C:\\Apps\.\browser.exe", r"c:\Apps\browser.exe")]
    #[test_case(
        r"\\\\server\\share\\Apps\.\browser.exe",
        r"\\server\share\Apps\browser.exe"
    )]
    #[test_case(r"C:\Äpps\Σ.exe", r"c:\äPPS\σ.exe"; "unicode_case")]
    fn equivalent(left: &str, right: &str) {
        let left = ExecutablePath::new(left).expect("valid absolute path");
        let right = ExecutablePath::new(right).expect("valid absolute path");
        assert_eq!(left, right);
        assert_eq!(left.cmp(&right), Ordering::Equal);
    }

    #[test]
    fn invalid_paths_are_rejected() {
        for path in [
            "",
            r"C:app.exe",
            r"\app.exe",
            r"app.exe",
            r"\\?\GLOBALROOT\app.exe",
            r"\\?\\\server\share\app.exe",
            r"\\.\C:\app.exe",
            r"C:\..\app.exe",
            r"\\server\share\..\app.exe",
            r"C:\Apps.\app.exe",
            r"C:\Apps \app.exe",
            r"C:\NUL\app.exe",
            r"C:\COM¹.txt\app.exe",
            r"C:\lpt9\app.exe",
            r"C:\A?\app.exe",
            "C:\\A\u{1f}\\app.exe",
            r"C:\app.exe\",
            r"C:\app.exe\.",
            r"C:\app.exe\..",
            r"C:\app.txt",
            r"\\server\app.exe",
        ] {
            assert_eq!(
                ExecutablePath::new(path).err(),
                Some(InvalidExecutablePath),
                "{path:?}"
            );
        }
    }

    #[test]
    fn missing_file_is_valid_and_distinct_installations_order_consistently() {
        let a = ExecutablePath::new(r"Q:\Missing\A\app.exe").expect("lexical path");
        let b = ExecutablePath::new(r"Q:\Missing\B\app.exe").expect("lexical path");
        assert!(a < b);
        assert_eq!(a.file_name(), "app.exe");
        assert_eq!(a.folder(), r"Q:\Missing\A");
    }
}
