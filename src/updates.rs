use serde::Deserialize;
use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

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
    Failed,
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
    status: Status,
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
                status: Status::Idle,
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

    pub(crate) fn status(&self) -> Status {
        self.shared
            .0
            .lock()
            .map_or(Status::Failed, |shared| shared.status.clone())
    }

    pub(crate) fn configure(&self, automatic: bool) {
        if let Ok(mut shared) = self.shared.0.lock() {
            shared.automatic = automatic;
            self.shared.1.notify_one();
        }
    }

    pub(crate) fn check(&self) {
        if let Ok(mut shared) = self.shared.0.lock() {
            if shared.status == Status::Checking {
                return;
            }
            shared.requested = true;
            shared.status = Status::Checking;
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
        while !(state.stopped || state.requested || state.automatic && Instant::now() >= due) {
            let waited = if state.automatic {
                shared
                    .1
                    .wait_timeout(state, due.saturating_duration_since(Instant::now()))
                    .map(|(state, _)| state)
                    .ok()
            } else {
                shared.1.wait(state).ok()
            };
            let Some(next) = waited else {
                return;
            };
            state = next;
        }
        if state.stopped {
            return;
        }
        state.requested = false;
        state.status = Status::Checking;
        drop(state);
        wake();
        let result = check().unwrap_or_else(|error| {
            tracing::warn!(%error, "checking for application updates failed");
            Status::Failed
        });
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        state.status = result;
        due = Instant::now() + DAILY;
        drop(state);
        wake();
    }
}

struct Api {
    agent: ureq::Agent,
    deadline: Instant,
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
        let mut response = match response {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(error) => return Err(io::Error::other(error)),
        };
        let body = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(io::Error::other)?;
        serde_json::from_str(&body)
            .map(Some)
            .map_err(io::Error::other)
    }
}

fn check() -> io::Result<Status> {
    let api = Api {
        agent: ureq::Agent::config_builder()
            .https_only(true)
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
        for artifact in artifacts(run.id)?
            .into_iter()
            .flat_map(|list| list.artifacts)
        {
            if artifact.expired || artifact.size_in_bytes == 0 {
                continue;
            }
            let Some(source) = artifact.name.strip_prefix("Circadia-windows-x64-unsigned-") else {
                continue;
            };
            if source.len() != 40
                || !source.bytes().all(|byte| byte.is_ascii_hexdigit())
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
