use serde::Deserialize;
use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

pub(crate) const REPOSITORY: &str = "https://github.com/kvnxiao/circadia";
const DAILY: Duration = Duration::from_hours(24);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Status {
    #[default]
    Idle,
    Checking,
    Current,
    Available(String),
    Unpublished,
    Development,
    RateLimited(Option<Instant>),
    ConnectionFailed,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) status: Status,
    pub(crate) dismissed: bool,
}

impl State {
    pub(crate) fn can_check(&self) -> bool {
        !matches!(self.status, Status::Checking | Status::RateLimited(Some(_)))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("update checks rate limited for {0:?}")]
struct RateLimit(Duration);

fn failure_status(error: &io::Error) -> Status {
    if let Some(limit) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<RateLimit>())
    {
        return Status::RateLimited(Some(Instant::now() + limit.0));
    }
    let network = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<ureq::Error>());
    if matches!(
        network,
        Some(
            ureq::Error::Io(_)
                | ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed
        )
    ) || error.kind() == io::ErrorKind::TimedOut
    {
        Status::ConnectionFailed
    } else {
        Status::Failed
    }
}

pub(crate) fn version() -> String {
    let suffix = match env!("CIRCADIA_CHANNEL") {
        "stable" => "",
        "preview" => "-preview",
        _ => "-dev",
    };
    format!("{}{suffix}", env!("CARGO_PKG_VERSION"))
}

struct Shared {
    automatic: bool,
    requested: bool,
    stopped: bool,
    state: State,
}

pub(crate) struct Client {
    shared: Arc<(Mutex<Shared>, Condvar)>,
}

impl Client {
    pub(crate) fn spawn(automatic: bool, wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        let shared = Arc::new((
            Mutex::new(Shared {
                automatic,
                requested: false,
                stopped: false,
                state: State::default(),
            }),
            Condvar::new(),
        ));
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                run(&worker, &wake, check);
            })?;
        Ok(Self { shared })
    }

    pub(crate) fn status(&self) -> State {
        self.shared.0.lock().map_or(
            State {
                status: Status::Failed,
                dismissed: false,
            },
            |shared| shared.state.clone(),
        )
    }

    pub(crate) fn dismiss(&self) {
        if let Ok(mut shared) = self.shared.0.lock()
            && !matches!(shared.state.status, Status::Idle | Status::Checking)
        {
            shared.state.dismissed = true;
        }
    }

    pub(crate) fn configure(&self, automatic: bool) {
        if let Ok(mut shared) = self.shared.0.lock()
            && shared.automatic != automatic
        {
            shared.automatic = automatic;
            self.shared.1.notify_one();
        }
    }

    pub(crate) fn check(&self) {
        if let Ok(mut shared) = self.shared.0.lock() {
            if !shared.state.can_check() {
                return;
            }
            shared.requested = true;
            shared.state = State {
                status: Status::Checking,
                dismissed: false,
            };
            self.shared.1.notify_one();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.shared.0.lock() {
            shared.stopped = true;
            self.shared.1.notify_one();
        }
        // An in-flight read has a deadline and cannot modify application data.
        // Process exit must not wait for the network before Night light
        // cleanup.
    }
}

fn run(
    shared: &(Mutex<Shared>, Condvar),
    wake: &impl Fn(),
    mut check: impl FnMut() -> io::Result<Status>,
) {
    let mut due = Instant::now();
    loop {
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if let Status::RateLimited(Some(until)) = state.state.status
            && Instant::now() >= until
        {
            state.state.status = Status::RateLimited(None);
            drop(state);
            wake();
            continue;
        }
        let cooldown = match state.state.status {
            Status::RateLimited(Some(until)) => Some(until),
            _ => None,
        };
        if !(state.stopped
            || state.requested
            || state.automatic && Instant::now() >= due && cooldown.is_none())
        {
            let deadline = cooldown.or(state.automatic.then_some(due));
            let waited = if let Some(deadline) = deadline {
                shared
                    .1
                    .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                    .map(|(state, _)| state)
                    .ok()
            } else {
                shared.1.wait(state).ok()
            };
            let Some(next) = waited else {
                return;
            };
            drop(next);
            continue;
        }
        if state.stopped {
            return;
        }
        state.requested = false;
        state.state = State {
            status: Status::Checking,
            dismissed: false,
        };
        drop(state);
        wake();
        let result = check().unwrap_or_else(|error| {
            tracing::warn!(%error, "checking for application updates failed");
            failure_status(&error)
        });
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        state.state.status = result;
        due = Instant::now() + DAILY;
        drop(state);
        wake();
    }
}

struct Api {
    agent: ureq::Agent,
    deadline: Instant,
}

fn rate_limit(
    status: u16,
    headers: &ureq::http::HeaderMap,
    body: &str,
    now: SystemTime,
) -> Option<Duration> {
    if !matches!(status, 403 | 429) {
        return None;
    }
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let exhausted = header("x-ratelimit-remaining") == Some("0");
    let message = serde_json::from_str::<serde_json::Value>(body).ok();
    let limited = message
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|message| message.to_ascii_lowercase().contains("rate limit"));
    if status != 429
        && !(status == 403 && (exhausted || header("retry-after").is_some() || limited))
    {
        return None;
    }
    let retry = header("retry-after")
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    let reset = exhausted
        .then(|| header("x-ratelimit-reset"))
        .flatten()
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|seconds| SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
        .and_then(|reset| reset.duration_since(now).ok());
    Some(
        retry
            .into_iter()
            .chain(reset)
            .filter(|delay| !delay.is_zero() && Instant::now().checked_add(*delay).is_some())
            .max()
            .unwrap_or(Duration::from_secs(60)),
    )
}

impl Api {
    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> io::Result<Option<T>> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "update check deadline",
            ));
        }
        let response = self
            .agent
            .get(format!(
                "https://api.github.com/repos/kvnxiao/circadia/{path}"
            ))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .config()
            .timeout_global(Some(remaining))
            .build()
            .call();
        let mut response = response.map_err(io::Error::other)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if let Some(delay) = rate_limit(status, response.headers(), "", SystemTime::now()) {
            return Err(io::Error::other(RateLimit(delay)));
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(io::Error::other)?;
        if let Some(delay) = rate_limit(status, response.headers(), &body, SystemTime::now()) {
            return Err(io::Error::other(RateLimit(delay)));
        }
        if !response.status().is_success() {
            return Err(io::Error::other(ureq::Error::StatusCode(status)));
        }
        serde_json::from_str(&body)
            .map(Some)
            .map_err(io::Error::other)
    }
}

fn check() -> io::Result<Status> {
    let api = Api {
        agent: ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .max_redirects(0)
            .user_agent(concat!("Circadia/", env!("CARGO_PKG_VERSION")))
            .build()
            .into(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    match env!("CIRCADIA_CHANNEL") {
        "stable" => {
            let release = api.get::<Release>("releases/latest")?;
            stable(release.as_ref(), env!("CARGO_PKG_VERSION"))
        }
        "preview" => preview(&api),
        _ => Ok(Status::Development),
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
}

fn stable(release: Option<&Release>, installed: &str) -> io::Result<Status> {
    let Some(release) = release.filter(|release| !release.draft && !release.prerelease) else {
        return Ok(Status::Unpublished);
    };
    let tag = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let latest = semver::Version::parse(tag).map_err(io::Error::other)?;
    let installed = semver::Version::parse(installed).map_err(io::Error::other)?;
    if !latest.pre.is_empty() {
        return Ok(Status::Unpublished);
    }
    if latest <= installed {
        return Ok(Status::Current);
    }
    let package = release.assets.iter().find(|asset| {
        asset.size > 0
            && asset.name.starts_with(&format!("Circadia-{tag}-"))
            && asset.name.ends_with("-windows-x64-unsigned-setup.exe")
    });
    let complete = package.is_some_and(|package| {
        release
            .assets
            .iter()
            .any(|asset| asset.size > 0 && asset.name == format!("{}.sha256", package.name))
    });
    Ok(if complete {
        Status::Available(format!("{REPOSITORY}/releases/tag/{}", release.tag_name))
    } else {
        Status::Unpublished
    })
}

#[derive(Deserialize)]
struct Runs {
    workflow_runs: Vec<Run>,
}
#[derive(Deserialize)]
struct Run {
    id: u64,
    #[serde(rename = "run_number")]
    number: u64,
    event: String,
    conclusion: Option<String>,
    head_branch: String,
}
#[derive(Deserialize)]
struct Artifacts {
    artifacts: Vec<Artifact>,
}
#[derive(Deserialize)]
struct Artifact {
    name: String,
    expired: bool,
    size_in_bytes: u64,
}

fn preview(api: &Api) -> io::Result<Status> {
    let Some(runs) = api.get::<Runs>(
        "actions/workflows/preview.yml/runs?event=push&status=success&branch=main&per_page=20",
    )?
    else {
        return Ok(Status::Unpublished);
    };
    preview_packages(
        runs,
        |id| api.get::<Artifacts>(&format!("actions/runs/{id}/artifacts?per_page=100")),
        |source| {
            api.get::<Comparison>(&format!(
                "compare/{}...{source}?per_page=1",
                env!("CIRCADIA_SOURCE_COMMIT")
            ))?
            .ok_or_else(|| io::Error::other("packaged source commit is unavailable"))
        },
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Relation {
    Ahead,
    Behind,
    Identical,
    Diverged,
}

#[derive(Deserialize)]
struct Comparison {
    status: Relation,
    ahead_by: u64,
}

fn preview_packages(
    mut runs: Runs,
    mut artifacts: impl FnMut(u64) -> io::Result<Option<Artifacts>>,
    mut compare: impl FnMut(&str) -> io::Result<Comparison>,
) -> io::Result<Status> {
    runs.workflow_runs
        .sort_by_key(|run| std::cmp::Reverse(run.number));
    let mut inspected = std::collections::BTreeSet::new();
    let mut newest = None;
    let mut current = false;
    let mut diverged = false;
    for run in runs.workflow_runs {
        if run.event != "push"
            || run.conclusion.as_deref() != Some("success")
            || run.head_branch != "main"
        {
            continue;
        }
        let packages: Vec<_> = artifacts(run.id)?
            .into_iter()
            .flat_map(|list| list.artifacts)
            .filter(|artifact| !artifact.expired && artifact.size_in_bytes > 0)
            .collect();
        for artifact in &packages {
            let legacy = artifact
                .name
                .strip_prefix("Circadia-windows-x64-unsigned-")
                .filter(|source| source.len() == 40);
            let separate = artifact
                .name
                .strip_prefix("Circadia-")
                .and_then(|name| name.strip_suffix("-windows-x64-unsigned-setup.exe"))
                .and_then(|name| name.rsplit_once("-preview-"))
                .filter(|(version, source)| {
                    semver::Version::parse(version).is_ok()
                        && (12..=40).contains(&source.len())
                        && packages
                            .iter()
                            .any(|checksum| checksum.name == format!("{}.sha256", artifact.name))
                })
                .map(|(_, source)| source);
            let Some(source) = legacy.or(separate) else {
                continue;
            };
            if !source.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !inspected.insert(source.to_owned())
            {
                continue;
            }
            let relation = compare(source)?;
            match relation.status {
                Relation::Ahead if relation.ahead_by > 0 => {
                    newest = newest
                        .filter(|(distance, _)| *distance >= relation.ahead_by)
                        .or(Some((relation.ahead_by, run.id)));
                }
                Relation::Identical | Relation::Behind => current = true,
                Relation::Diverged | Relation::Ahead => diverged = true,
            }
        }
    }
    Ok(if let Some((_, id)) = newest {
        Status::Available(format!("{REPOSITORY}/actions/runs/{id}"))
    } else if diverged {
        Status::Failed
    } else if current {
        Status::Current
    } else {
        Status::Unpublished
    })
}

#[cfg(test)]
mod tests;
