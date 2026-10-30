use std::io::Write;

pub(crate) fn write(line: &str) {
    let Ok(path) = std::env::current_exe().map(|path| path.with_extension("log")) else {
        return;
    };
    if let Ok(mut file) = fs_err::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _written = writeln!(file, "{line}");
    }
}
