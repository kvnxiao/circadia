use std::io;
use std::path::Path;
use std::path::PathBuf;
use winsafe::HKEY;
use winsafe::RegistryValue;
use winsafe::co;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const NAME: &str = "Circadia";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effective {
    NotRegistered,
    Enabled,
    DisabledByWindows,
    DifferentCommand,
}

pub(crate) struct Registration {
    executable: PathBuf,
    directory: Option<PathBuf>,
}

impl Registration {
    pub(crate) fn new(executable: PathBuf, directory: Option<PathBuf>) -> Self {
        Self {
            executable,
            directory,
        }
    }

    pub(crate) fn read(&self) -> io::Result<Effective> {
        let Some(value) = read_value(RUN)? else {
            return Ok(Effective::NotRegistered);
        };
        if !self.registers(&value)? {
            return Ok(Effective::DifferentCommand);
        }
        if approved(read_value(APPROVED)?)? {
            Ok(Effective::Enabled)
        } else {
            Ok(Effective::DisabledByWindows)
        }
    }

    pub(crate) fn set(&self, enabled: bool) -> io::Result<()> {
        if enabled {
            let command = self.command()?;
            let (key, _disposition) = HKEY::CURRENT_USER
                .RegCreateKeyEx(
                    RUN,
                    None,
                    co::REG_OPTION::NON_VOLATILE,
                    co::KEY::SET_VALUE,
                    None,
                )
                .map_err(windows_error)?;
            key.RegSetValueEx(Some(NAME), RegistryValue::Sz(command))
                .map_err(windows_error)
        } else {
            let key = match HKEY::CURRENT_USER.RegOpenKeyEx(
                Some(RUN),
                co::REG_OPTION::default(),
                co::KEY::SET_VALUE,
            ) {
                Ok(key) => key,
                Err(co::ERROR::FILE_NOT_FOUND | co::ERROR::PATH_NOT_FOUND) => return Ok(()),
                Err(error) => return Err(windows_error(error)),
            };
            match key.RegDeleteValue(Some(NAME)) {
                Ok(()) | Err(co::ERROR::FILE_NOT_FOUND) => Ok(()),
                Err(error) => Err(windows_error(error)),
            }
        }
    }

    fn registers(&self, value: &RegistryValue) -> io::Result<bool> {
        Ok(matches!(value, RegistryValue::Sz(command) if *command == self.command()?))
    }

    fn command(&self) -> io::Result<String> {
        let executable = quote_path(&self.executable)?;
        let command = match &self.directory {
            Some(directory) => format!("{executable} --data-dir {}", quote_path(directory)?),
            None => executable,
        };
        // Microsoft limits Run commands to 260 characters without stating
        // whether the terminating null counts, so 259 is the longest safe
        // length.
        if command.encode_utf16().count() > 259 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "startup command exceeds 259 characters; use shorter application and data directory paths",
            ));
        }
        Ok(command)
    }
}

fn quote_path(path: &Path) -> io::Result<String> {
    let path = std::path::absolute(path)?;
    let text = path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "startup path is not Unicode")
    })?;
    if text.contains(['\0', '"']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "startup path has an invalid character",
        ));
    }
    let trailing = text
        .chars()
        .rev()
        .take_while(|&value| value == '\\')
        .count();
    Ok(format!("\"{text}{}\"", "\\".repeat(trailing)))
}

fn read_value(key: &str) -> io::Result<Option<RegistryValue>> {
    match HKEY::CURRENT_USER.RegGetValue(Some(key), Some(NAME), co::RRF::RT_ANY | co::RRF::NOEXPAND)
    {
        Ok(value) => Ok(Some(value)),
        Err(co::ERROR::FILE_NOT_FOUND | co::ERROR::PATH_NOT_FOUND) => Ok(None),
        Err(error) => Err(windows_error(error)),
    }
}

fn approved(value: Option<RegistryValue>) -> io::Result<bool> {
    let Some(value) = value else { return Ok(true) };
    if let RegistryValue::Binary(bytes) = value
        && let [state, 0, 0, 0, _, _, _, _, _, _, _, _] = bytes.as_slice()
    {
        match state {
            0 | 2 => return Ok(true),
            3 => return Ok(false),
            _ => {}
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Windows startup approval has an unrecognized format; check Windows Settings > Apps > Startup",
    ))
}

fn windows_error(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(i32::from_ne_bytes(error.raw().to_ne_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn absent_approval_allows_a_registered_command() {
        assert!(approved(None).expect("absent approval"));
    }

    #[test_case(0, true; "unrestricted")]
    #[test_case(2, true; "enabled")]
    #[test_case(3, false; "disabled")]
    fn approval_preserves_windows_enablement(state: u8, expected: bool) {
        let value = RegistryValue::Binary(vec![state, 0, 0, 0, 12, 34, 56, 78, 90, 12, 34, 56]);
        assert_eq!(
            approved(Some(value)).expect("known approval state"),
            expected
        );
    }

    #[test_case(RegistryValue::Binary(vec![]); "empty")]
    #[test_case(RegistryValue::Binary(vec![2]); "short")]
    #[test_case(RegistryValue::Binary(vec![2; 12]); "invalid state word")]
    #[test_case(RegistryValue::Binary(vec![9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); "unknown state")]
    #[test_case(RegistryValue::Binary(vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); "oversized")]
    #[test_case(RegistryValue::Dword(2); "wrong registry type")]
    fn unrecognized_approval_does_not_claim_enabled(value: RegistryValue) {
        assert_eq!(
            approved(Some(value)).expect_err("unknown approval").kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn command_with_the_default_data_directory_is_the_quoted_executable() {
        let registration = Registration::new(
            PathBuf::from(r"C:\Program Files\Circadia\circadia.exe"),
            None,
        );
        assert_eq!(
            registration.command().expect("startup command"),
            r#""C:\Program Files\Circadia\circadia.exe""#
        );
    }

    #[test]
    fn command_with_a_custom_data_directory_quotes_both_paths() {
        let registration = Registration::new(
            PathBuf::from(r"C:\Program Files\Circadia\circadia.exe"),
            Some(PathBuf::from(r"D:\Circadia data")),
        );
        assert_eq!(
            registration.command().expect("startup command"),
            r#""C:\Program Files\Circadia\circadia.exe" --data-dir "D:\Circadia data""#
        );
        assert_eq!(
            quote_path(Path::new(r"D:\")).expect("root path"),
            r#""D:\\""#
        );
    }

    #[test]
    fn a_registered_executable_only_command_matches_the_default_registration() {
        let registration = Registration::new(PathBuf::from(r"C:\Circadia\circadia.exe"), None);
        assert!(
            registration
                .registers(&RegistryValue::Sz(
                    r#""C:\Circadia\circadia.exe""#.to_owned()
                ))
                .expect("comparable command")
        );
        assert!(
            !registration
                .registers(&RegistryValue::Sz(
                    r#""C:\Circadia\circadia.exe" --data-dir "C:\Users\a\AppData\Local\Circadia""#
                        .to_owned()
                ))
                .expect("comparable command")
        );
        assert!(
            !registration
                .registers(&RegistryValue::ExpandSz(
                    r#""C:\Circadia\circadia.exe""#.to_owned()
                ))
                .expect("comparable command")
        );
    }

    #[test_case(None, 259, true; "executable only at the limit")]
    #[test_case(None, 260, false; "executable only beyond the limit")]
    #[test_case(Some(r"D:\d"), 259, true; "custom directory at the limit")]
    #[test_case(Some(r"D:\d"), 260, false; "custom directory beyond the limit")]
    fn command_length_is_limited_to_259_characters(
        directory: Option<&str>,
        length: usize,
        accepted: bool,
    ) {
        let suffix = directory.map_or(String::new(), |directory| {
            format!(r#" --data-dir "{directory}""#)
        });
        let name = "a".repeat(length - suffix.len() - r#""C:\.exe""#.len());
        let executable = format!(r"C:\{name}.exe");
        let expected = format!("\"{executable}\"{suffix}");
        assert_eq!(expected.encode_utf16().count(), length);
        let command =
            Registration::new(PathBuf::from(&executable), directory.map(PathBuf::from)).command();
        assert_eq!(
            command.map_err(|error| error.kind()),
            if accepted {
                Ok(expected)
            } else {
                Err(io::ErrorKind::InvalidInput)
            }
        );
    }
}
