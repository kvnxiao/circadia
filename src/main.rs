//! Run the Circadia tray application in the current Windows session.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    directory: Option<std::path::PathBuf>,
    verbose: bool,
    language: Option<String>,
}

#[cfg(windows)]
fn parse(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> std::io::Result<Arguments> {
    let usage = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: circadia [--data-dir DIRECTORY] [--verbose | -V]",
        )
    };
    let mut arguments = arguments.into_iter();
    let mut parsed = Arguments {
        directory: None,
        verbose: false,
        language: None,
    };
    while let Some(argument) = arguments.next() {
        if argument == "--verbose" || argument == "-V" {
            parsed.verbose = true;
            continue;
        }
        if argument == "--language" && parsed.language.is_none() {
            parsed.language = Some(
                arguments
                    .next()
                    .and_then(|tag| tag.into_string().ok())
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "--language requires a language tag",
                        )
                    })?,
            );
            continue;
        }
        if argument != "--data-dir" || parsed.directory.is_some() {
            return Err(usage());
        }
        parsed.directory = Some(std::path::PathBuf::from(arguments.next().ok_or_else(
            || {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--data-dir requires a directory",
                )
            },
        )?));
    }
    Ok(parsed)
}

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    let arguments = parse(std::env::args_os().skip(1))?;
    circadia::run_desktop(
        arguments.directory,
        arguments.verbose,
        arguments.language.as_deref(),
    )
}

#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Circadia requires Windows 11",
    ))
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use test_case::test_case;

    #[test_case(&[], None, false, None; "no arguments")]
    #[test_case(&["--verbose"], None, true, None; "long verbose flag")]
    #[test_case(&["-V"], None, true, None; "short verbose flag")]
    #[test_case(&["--data-dir", "D:\\data"], Some("D:\\data"), false, None; "data directory")]
    #[test_case(&["-V", "--data-dir", "D:\\data"], Some("D:\\data"), true, None; "flag before directory")]
    #[test_case(&["--data-dir", "D:\\data", "--verbose"], Some("D:\\data"), true, None; "flag after directory")]
    #[test_case(&["--language", "qps-Ploc"], None, false, Some("qps-Ploc"); "forced language")]
    #[test_case(&["--language", "xx-XX", "-V"], None, true, Some("xx-XX"); "unvalidated language")]
    fn accepted_arguments(
        arguments: &[&str],
        directory: Option<&str>,
        verbose: bool,
        language: Option<&str>,
    ) {
        assert_eq!(
            parse(arguments.iter().map(Into::into)).expect("accepted arguments"),
            Arguments {
                directory: directory.map(PathBuf::from),
                verbose,
                language: language.map(str::to_owned),
            }
        );
    }

    #[test_case(&["--quiet"]; "unknown flag")]
    #[test_case(&["-v"]; "lowercase short flag")]
    #[test_case(&["--data-dir"]; "missing directory")]
    #[test_case(&["--data-dir", "C:\\a", "--data-dir", "C:\\b"]; "repeated directory")]
    #[test_case(&["--verbose", "extra"]; "positional argument")]
    #[test_case(&["--language"]; "missing language")]
    #[test_case(&["--language", "en-US", "--language", "qps-Ploc"]; "repeated language")]
    fn rejected_arguments(arguments: &[&str]) {
        assert_eq!(
            parse(arguments.iter().map(Into::into))
                .expect_err("rejected arguments")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}
