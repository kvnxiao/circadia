---
paths: **/*.{py,pyi}
description: "Python 3.15+ text and file handling; the UTF-8 default from PEP 686, explicit non-UTF-8 encodings, subprocess output encodings on Windows, the bytes and str boundary, pathlib, line endings, atomic writes, and locale-independent text processing."
---

# Text Encoding and Files

## Rely on the UTF-8 default for text (Required)

Python 3.15 enables UTF-8 mode by default on every platform
([PEP 686](https://peps.python.org/pep-0686/)). `open()`, `Path.read_text()`, `Path.write_text()`,
standard streams, and `subprocess` text mode use UTF-8 when no encoding is given. On Windows, this
replaces the ANSI code page, such as cp1252, as the default for text files.

- Omit `encoding` for UTF-8 text. Do not write `encoding=locale.getpreferredencoding()`, which
  returns `"utf-8"` in UTF-8 mode.
- Pass `encoding="utf-8"` explicitly when the output must be byte-identical under every interpreter
  configuration, such as a generated lockfile, a file that is hashed, or a file in a format that
  requires UTF-8. Whoever runs the program can turn UTF-8 mode off with `PYTHONUTF8=0` or `-X
  utf8=0`, which restores the locale encoding as the default.
- Pass the real encoding when data is not UTF-8:
  - `encoding="utf-8-sig"` for files that may start with a byte order mark, such as CSV files
    exported by spreadsheet programs.
  - `encoding="locale"` for files that other local programs write in the system locale encoding.
    `locale.getencoding()` returns that encoding by name.
  - A named codec such as `"cp1252"` for legacy files with a known encoding.

## Decode subprocess output with the child's encoding (Required)

`text=True` decodes a child process's output as UTF-8, but the child chooses the encoding it writes.
On Windows, many console programs write in the OEM code page, such as cp437 or cp850. Pass the
child's actual encoding when it is not UTF-8, or capture bytes and decode them explicitly:

```python
ipconfig = shutil.which("ipconfig")
if ipconfig is None:
    raise ToolNotFoundError(program="ipconfig")
result = subprocess.run(  # noqa: S603 -- argv is a fixed command
    [ipconfig, "/all"], check=True, capture_output=True, encoding="oem", timeout=10
)
```

The `"oem"` codec exists only on Windows. Child processes running Python 3.15 or later write UTF-8
unless UTF-8 mode is turned off in the child.

## Keep bytes and text separate (Required)

Decode bytes to `str` where data enters the program and encode `str` to bytes where it leaves. Do
not pass bytes through text-processing code or concatenate bytes with text.

- Keep the default `errors="strict"` so that invalid input raises instead of producing replacement
  characters.
- Use `errors="surrogateescape"` only to round-trip undecodable data, such as file names, back to
  bytes unchanged.
- Read binary files in binary mode (`"rb"`). Opening a binary file in text mode raises
  `UnicodeDecodeError` on the first invalid byte.

## Use pathlib for paths (Default)

Ruff's `PTH` rules replace `os.path` calls with `pathlib`. Default function signatures to accept and
return `pathlib.Path`, and convert strings to `Path` at the boundary where they enter the program.
Build paths with the `/` operator, not string formatting. For user-supplied paths, apply the
[containment rule](python-defensive-programming.md#contain-user-supplied-paths-required).

## Control line endings for generated files (Default)

Text mode translates `"\n"` to the platform line separator on write, which is `"\r\n"` on Windows.
Default generated files that are committed to version control or compared byte for byte to
`newline="\n"`:

```python
destination.write_text(rendered, newline="\n")
```

Open files for the `csv` module with `newline=""`, as its documentation requires. Without it, the
module writes an extra blank line between rows on Windows.

## Write files atomically (Default)

Default writes that replace an existing file to a temporary file in the same directory followed by
`os.replace`. A reader then sees either the old file or the new file, never a partial write:

```python
def write_atomic(path: Path, data: bytes) -> None:
    descriptor, name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    temp_path = Path(name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        if path.exists():
            shutil.copymode(path, temp_path)
        temp_path.replace(path)
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise
```

`tempfile.mkstemp` creates the file readable only by its owner, so the example copies the existing
file's permission bits before replacing it. On Windows, `os.replace` raises `PermissionError` when
another process has the target open without the `FILE_SHARE_DELETE` sharing mode. Retry or report
the error; do not fall back to writing the target in place.

## Process text without depending on the locale (Default)

- Do not call `locale.setlocale` in library code. It changes process-wide state that affects every
  thread.
- Compare text case-insensitively with `str.casefold()`, not `str.lower()`.
- Normalize user-entered text with `unicodedata.normalize("NFC", text)` before comparing or storing
  it, so composed and decomposed forms of the same character compare equal.
- Format numbers, dates, and currencies for display with `babel` when the application supports
  several locales. Store and exchange them in locale-independent formats such as ISO 8601.
