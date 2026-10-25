use jiff::SignedDuration;
use jiff::Timestamp;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreviousRun {
    Normal,
    Crashed(CrashSummary),
    Unexplained,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CrashSummary {
    pub(crate) time: Timestamp,
    pub(crate) description: String,
    pub(crate) module: Option<String>,
    pub(crate) offset: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CrashEvent {
    time: Timestamp,
    process_id: u32,
    code: String,
    module: String,
    offset: String,
    report_id: Option<String>,
}

fn marker_path(data_directory: &Path) -> PathBuf {
    data_directory.join("logs").join("run.marker")
}

pub(crate) fn begin(data_directory: &Path, executable: &str) -> PreviousRun {
    let previous = previous_run(
        data_directory,
        executable,
        crate::platform::event_log::query,
    );
    let marker = marker_path(data_directory);
    let written = marker
        .parent()
        .map_or(Ok(()), fs_err::create_dir_all)
        .and_then(|()| {
            fs_err::write(
                &marker,
                format!(
                    "process_id={}\nstarted={:.6}\n",
                    std::process::id(),
                    Timestamp::now()
                ),
            )
        });
    if let Err(error) = written {
        tracing::warn!(error = %error, "run marker write failed");
    }
    previous
}

pub(crate) fn finish(data_directory: &Path) {
    match fs_err::remove_file(marker_path(data_directory)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            tracing::warn!(error = %error, "run marker removal failed");
        }
        _ => {}
    }
}

fn previous_run(
    data_directory: &Path,
    executable: &str,
    mut query: impl FnMut(&str) -> io::Result<Vec<String>>,
) -> PreviousRun {
    let Ok(marker) = fs_err::read_to_string(marker_path(data_directory)) else {
        return PreviousRun::Normal;
    };
    let value = |key: &str| {
        marker
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    };
    let started_text = value("started").unwrap_or_default();
    let (Some(process_id), Ok(started)) = (
        value("process_id").and_then(|id| id.parse::<u32>().ok()),
        started_text.parse::<Timestamp>(),
    ) else {
        tracing::info!(marker = %marker.trim_end(), "previous run ended unexpectedly");
        return PreviousRun::Unexplained;
    };
    // System time can move backward after the marker is written, such as a
    // time-sync correction after sign-in.
    let earliest = started
        .checked_sub(SignedDuration::from_mins(10))
        .unwrap_or(started);
    if let Some(summary) = logged_crash(data_directory, process_id, earliest) {
        return PreviousRun::Crashed(summary);
    }
    let mut query_error = None;
    let event = match event_query(executable, &format!("{earliest:.6}")) {
        Some(xpath) => match query(&xpath) {
            Ok(events) => events
                .iter()
                .filter_map(|event| crash_event(event))
                .find(|event| event.process_id == process_id && event.time >= earliest),
            Err(error) => {
                query_error = Some(error);
                None
            }
        },
        None => None,
    };
    let Some(event) = event else {
        tracing::info!(
            process_id,
            started = %started_text,
            event_log_error = query_error.as_ref().map(tracing::field::display),
            "previous run ended unexpectedly"
        );
        return PreviousRun::Unexplained;
    };
    let fast_fail = event.report_id.as_deref().and_then(|report_id| {
        let xpath = format!(
            "*[System[Provider[@Name='Windows Error Reporting'] and (EventID=1001)]] and *[EventData[Data[@Name='ReportId']='{report_id}']]"
        );
        query(&xpath)
            .ok()?
            .iter()
            .find_map(|report| data(report, "P9"))
            .and_then(|code| u64::from_str_radix(&code, 16).ok())
            .filter(|code| *code != 0)
    });
    tracing::warn!(
        process_id,
        time = %event.time,
        code = %event.code,
        module = %event.module,
        offset = %event.offset,
        fast_fail,
        "previous run crashed"
    );
    PreviousRun::Crashed(CrashSummary {
        time: event.time,
        description: event.code,
        module: Some(event.module),
        offset: Some(event.offset),
    })
}

fn event_query(executable: &str, earliest: &str) -> Option<String> {
    let executable = if executable.contains('\'') {
        (!executable.contains('"')).then(|| format!("\"{executable}\""))?
    } else {
        format!("'{executable}'")
    };
    Some(format!(
        "*[System[Provider[@Name='Application Error'] and (EventID=1000) and TimeCreated[@SystemTime>='{earliest}']]] and *[EventData[Data[@Name='AppName']={executable}]]"
    ))
}

fn logged_crash(
    data_directory: &Path,
    process_id: u32,
    earliest: Timestamp,
) -> Option<CrashSummary> {
    let logs = data_directory.join("logs");
    let text = ["circadia.1.log", "circadia.log"]
        .iter()
        .filter_map(|name| fs_err::read_to_string(logs.join(name)).ok())
        .collect::<String>();
    text.lines().rev().find_map(|line| {
        let (time, record) = line.split_once(' ')?;
        let time = time.parse::<Timestamp>().ok()?;
        let record = record.trim_start().strip_prefix("ERROR circadia::log: ")?;
        let (kind, fields) = record.split_once(' ')?;
        if time < earliest || field(fields, "process_id")?.parse::<u32>().ok()? != process_id {
            return None;
        }
        match kind {
            "panic" => Some(CrashSummary {
                time,
                description: quoted_first_line(fields, "message")?,
                module: None,
                offset: None,
            }),
            "exception" => Some(CrashSummary {
                time,
                description: field(fields, "code")?.to_owned(),
                module: field(fields, "module").map(str::to_owned),
                offset: field(fields, "offset").map(str::to_owned),
            }),
            _ => None,
        }
    })
}

fn field<'a>(fields: &'a str, key: &str) -> Option<&'a str> {
    fields
        .split(' ')
        .find_map(|field| field.strip_prefix(key)?.strip_prefix('='))
}

fn quoted_first_line(fields: &str, key: &str) -> Option<String> {
    let (_, rest) = fields.split_once(&format!(" {key}=\""))?;
    let mut line = String::new();
    let mut characters = rest.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Some(line),
            '\\' => match characters.next()? {
                'n' | 'r' => return Some(line),
                't' => line.push('\t'),
                '0' => line.push('\0'),
                'u' => {
                    let code = characters
                        .by_ref()
                        .skip(1)
                        .take_while(|character| *character != '}')
                        .collect::<String>();
                    line.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
                }
                escaped => line.push(escaped),
            },
            character => line.push(character),
        }
    }
    None
}

fn crash_event(event: &str) -> Option<CrashEvent> {
    let time = attribute(event, "SystemTime")?.parse().ok()?;
    let process_id = data(event, "ProcessId")?;
    let process_id = u32::from_str_radix(process_id.trim_start_matches("0x"), 16).ok()?;
    let code = u32::from_str_radix(&data(event, "ExceptionCode")?, 16).ok()?;
    let offset = u64::from_str_radix(&data(event, "FaultingOffset")?, 16).ok()?;
    Some(CrashEvent {
        time,
        process_id,
        code: format!("{code:#010x}"),
        module: data(event, "ModuleName")?,
        offset: format!("{offset:#x}"),
        report_id: data(event, "IntegratorReportId").filter(|id| !id.is_empty()),
    })
}

fn attribute<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let (_, rest) = xml.split_once(&format!("{name}='"))?;
    Some(rest.split_once('\'')?.0)
}

fn data(xml: &str, name: &str) -> Option<String> {
    let (_, rest) = xml.split_once(&format!("<Data Name='{name}'>"))?;
    let (value, _) = rest.split_once("</Data>")?;
    Some(
        value
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests;
