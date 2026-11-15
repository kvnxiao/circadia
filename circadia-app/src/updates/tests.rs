use super::*;
use test_case::test_case;

fn release(tag: &str) -> Release {
    Release {
        tag_name: tag.into(),
        draft: false,
        prerelease: false,
        assets: vec![
            Asset {
                name: format!(
                    "Circadia-{}-abcdef012345-windows-x64-unsigned-setup.exe",
                    tag.trim_start_matches('v')
                ),
                size: 100,
            },
            Asset {
                name: format!(
                    "Circadia-{}-abcdef012345-windows-x64-unsigned-setup.exe.sha256",
                    tag.trim_start_matches('v')
                ),
                size: 64,
            },
        ],
    }
}

#[test_case("v0.1.0", &Outcome::Current; "equal")]
#[test_case("v0.0.9", &Outcome::Current; "installed newer")]
#[test_case("v0.10.0", &Outcome::Available("https://github.com/kvnxiao/circadia/releases/tag/v0.10.0".into()); "numeric version comparison")]
#[test_case("v0.2.0-preview", &Outcome::Unpublished; "prerelease tag")]
fn stable_version_comparison(tag: &str, expected: &Outcome) {
    assert_eq!(
        stable(Some(&release(tag)), "0.1.0").expect("valid versions"),
        *expected
    );
}

#[test]
fn missing_release_and_incomplete_assets_do_not_claim_current_or_offer_downloads() {
    assert_eq!(
        stable(None, "0.1.0").expect("no release"),
        Outcome::Unpublished
    );
    let mut latest = release("v0.2.0");
    latest.assets.pop();
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("missing checksum"),
        Outcome::Unpublished
    );
    latest.assets.clear();
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("missing installer"),
        Outcome::Unpublished
    );
}

#[test]
fn draft_and_prerelease_flags_exclude_releases() {
    let mut latest = release("v0.2.0");
    latest.draft = true;
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("draft"),
        Outcome::Unpublished
    );
    latest.draft = false;
    latest.prerelease = true;
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("prerelease"),
        Outcome::Unpublished
    );
}

#[test]
fn malformed_version_is_an_error() {
    assert!(stable(Some(&release("not-a-version")), "0.1.0").is_err());
}

#[test]
fn automatic_update_preference_defaults_on_and_round_trips() {
    let input = crate::ConfigInput::default();
    assert!(input.check_updates_automatically);
    let text = toml::to_string(&input).expect("serialize defaults");
    assert!(text.contains("check_updates_automatically = true"));
    let parsed: crate::ConfigInput = toml::from_str(&text.replace(
        "check_updates_automatically = true",
        "check_updates_automatically = false",
    ))
    .expect("saved switch");
    assert!(!parsed.check_updates_automatically);
}

fn runs() -> Runs {
    serde_json::from_str(
        r#"{"workflow_runs":[
        {"id":100,"run_number":10,"event":"push","conclusion":"success","head_branch":"main"},
        {"id":120,"run_number":12,"event":"push","conclusion":"success","head_branch":"main"},
        {"id":110,"run_number":11,"event":"push","conclusion":"success","head_branch":"main"}
    ]}"#,
    )
    .expect("workflow fixture")
}

fn artifact(id: u64, expired: bool) -> Artifacts {
    Artifacts {
        artifacts: vec![Artifact {
            name: format!("Circadia-windows-x64-unsigned-{id:040x}"),
            expired,
            size_in_bytes: 100,
        }],
    }
}

#[test_case("abcdef012345", false, 64, &Outcome::Available("https://github.com/kvnxiao/circadia/actions/runs/120".into()); "complete pair")]
#[test_case("abcdef0123456789", false, 64, &Outcome::Available("https://github.com/kvnxiao/circadia/actions/runs/120".into()); "longer abbreviated commit")]
#[test_case("abcdef012345", true, 64, &Outcome::Unpublished; "expired checksum")]
#[test_case("abcdef012345", false, 0, &Outcome::Unpublished; "empty checksum")]
#[test_case("main", false, 64, &Outcome::Unpublished; "invalid commit")]
fn preview_recognizes_separate_installer_and_checksum(
    source: &str,
    expired: bool,
    checksum_size: u64,
    expected: &Outcome,
) {
    let name = format!("Circadia-0.1.0-preview-{source}-windows-x64-unsigned-setup.exe");
    let result = preview_packages(
        runs(),
        |_| {
            Ok(Some(Artifacts {
                artifacts: vec![
                    Artifact {
                        name: name.clone(),
                        expired: false,
                        size_in_bytes: 100,
                    },
                    Artifact {
                        name: format!("{name}.sha256"),
                        expired,
                        size_in_bytes: checksum_size,
                    },
                ],
            }))
        },
        |commit| {
            assert!(
                matches!(expected, Outcome::Available(_)),
                "incomplete or malformed package must not be compared"
            );
            assert_eq!(commit, source);
            ahead(commit)
        },
    )
    .expect("preview metadata");
    assert_eq!(result, *expected);
}

#[test_case("Circadia-0.1.0-preview-abcdef012345-windows-x64-unsigned-setup.exe"; "missing checksum")]
#[test_case("Circadia-0.1.0-preview-abcdef012345-windows-x64-unsigned-setup.exe.sha256"; "missing installer")]
fn preview_rejects_an_incomplete_separate_download(name: &str) {
    let result = preview_packages(
        runs(),
        |_| {
            Ok(Some(Artifacts {
                artifacts: vec![Artifact {
                    name: name.into(),
                    expired: false,
                    size_in_bytes: 100,
                }],
            }))
        },
        |_| panic!("incomplete package must not be compared"),
    )
    .expect("preview metadata");
    assert_eq!(result, Outcome::Unpublished);
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "fixture implements the fallible comparison contract"
)]
fn ahead(_: &str) -> io::Result<Comparison> {
    Ok(Comparison {
        status: Relation::Ahead,
        ahead_by: 1,
    })
}

#[test]
fn previews_select_the_newest_source_even_when_older_ci_is_rerun_later() {
    let mut requested = Vec::new();
    let result = preview_packages(
        runs(),
        |id| {
            requested.push(id);
            Ok(Some(artifact(id, false)))
        },
        |source| {
            Ok(match source {
                "0000000000000000000000000000000000000078" => Comparison {
                    status: Relation::Behind,
                    ahead_by: 0,
                },
                "000000000000000000000000000000000000006e" => Comparison {
                    status: Relation::Ahead,
                    ahead_by: 2,
                },
                _ => Comparison {
                    status: Relation::Ahead,
                    ahead_by: 5,
                },
            })
        },
    )
    .expect("preview metadata");
    assert_eq!(requested, [120, 110, 100]);
    assert_eq!(
        result,
        Outcome::Available("https://github.com/kvnxiao/circadia/actions/runs/100".into())
    );
}

#[test]
fn preview_does_not_offer_expired_deleted_or_malformed_artifacts() {
    assert_eq!(
        preview_packages(runs(), |id| Ok(Some(artifact(id, true))), ahead)
            .expect("expired packages"),
        Outcome::Unpublished
    );
    assert_eq!(
        preview_packages(runs(), |_| Ok(None), ahead).expect("deleted packages"),
        Outcome::Unpublished
    );
    let result = preview_packages(
        runs(),
        |_| {
            Ok(Some(Artifacts {
                artifacts: vec![Artifact {
                    name: "Circadia-windows-x64-unsigned-main".into(),
                    expired: false,
                    size_in_bytes: 100,
                }],
            }))
        },
        |_| panic!("invalid source must not be requested"),
    )
    .expect("invalid source suffix");
    assert_eq!(result, Outcome::Unpublished);
}

#[test]
fn identical_rebuilds_are_current_and_divergent_sources_are_not_offered() {
    let result = preview_packages(
        runs(),
        |id| Ok(Some(artifact(id, false))),
        |_| {
            Ok(Comparison {
                status: Relation::Identical,
                ahead_by: 0,
            })
        },
    )
    .expect("identical source");
    assert_eq!(result, Outcome::Current);
    let result = preview_packages(
        runs(),
        |id| Ok(Some(artifact(id, false))),
        |_| {
            Ok(Comparison {
                status: Relation::Diverged,
                ahead_by: 4,
            })
        },
    )
    .expect("divergent source");
    assert_eq!(result, Outcome::Failed);
}

#[test]
fn a_divergent_rerun_does_not_hide_an_available_descendant() {
    let result = preview_packages(
        runs(),
        |id| Ok(Some(artifact(id, false))),
        |source| {
            Ok(if source.ends_with("78") {
                Comparison {
                    status: Relation::Diverged,
                    ahead_by: 10,
                }
            } else {
                Comparison {
                    status: Relation::Ahead,
                    ahead_by: 2,
                }
            })
        },
    )
    .expect("mixed source history");
    assert_eq!(
        result,
        Outcome::Available("https://github.com/kvnxiao/circadia/actions/runs/110".into())
    );
}

#[test]
fn preview_rejects_untrusted_runs_and_propagates_network_failures() {
    let invalid = serde_json::from_str(
        r#"{"workflow_runs":[
        {"id":1,"run_number":1,"event":"pull_request","conclusion":"success","head_branch":"main"},
        {"id":2,"run_number":2,"event":"push","conclusion":"failure","head_branch":"main"},
        {"id":3,"run_number":3,"event":"push","conclusion":"success","head_branch":"other"},
        {"id":4,"run_number":4,"event":"workflow_run","conclusion":"success","head_branch":"main"},
        {"id":5,"run_number":5,"event":"workflow_dispatch","conclusion":"success","head_branch":"main"},
        {"id":6,"run_number":6,"event":"push","conclusion":"cancelled","head_branch":"main"},
        {"id":7,"run_number":7,"event":"push","conclusion":null,"head_branch":"main"}
    ]}"#,
    )
    .expect("invalid runs");
    assert_eq!(
        preview_packages(invalid, |_| panic!("excluded runs need no lookup"), ahead)
            .expect("excluded"),
        Outcome::Unpublished
    );
    let error = preview_packages(
        runs(),
        |_| Err(io::Error::new(io::ErrorKind::TimedOut, "fixture")),
        ahead,
    )
    .expect_err("network failure");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn manual_checks_coalesce_and_shutdown_discards_in_flight_results() {
    let shared = Arc::new((
        Mutex::new(Shared::new(false, Instant::now())),
        Condvar::new(),
    ));
    let client = Client {
        shared: Arc::clone(&shared),
    };
    let worker_shared = Arc::clone(&shared);
    let (notified, notifications) = std::sync::mpsc::channel();
    let (finish, finishes) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        run(
            &worker_shared,
            &|| {
                notified.send(()).expect("notification listener");
            },
            &Instant::now,
            || {
                finishes.recv().expect("controlled check completion");
                Ok(Outcome::Current)
            },
        );
    });
    client.check();
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("checking notification");
    client.check();
    assert!(!matches!(
        shared.0.lock().expect("state lock").phase,
        Phase::Requested
    ));
    assert_eq!(client.status().status, Status::Checking);
    drop(client);
    finish.send(()).expect("release network fixture");
    worker.join().expect("worker exits");
    assert!(notifications.try_recv().is_err());
    assert_eq!(shared.0.lock().expect("state lock").result, None);
}

#[test_case(429, None, None, None, "", Some(60); "429 without timing")]
#[test_case(403, Some("0"), None, Some("1120"), "", Some(120); "primary reset")]
#[test_case(403, Some("12"), Some("90"), None, "", Some(90); "secondary retry")]
#[test_case(403, Some("0"), Some("30"), Some("1120"), "", Some(120); "honor both deadlines")]
#[test_case(403, None, None, None, r#"{"message":"You have exceeded a secondary rate limit."}"#, Some(60); "secondary body")]
#[test_case(403, None, None, None, r#"{"message":"Forbidden"}"#, None; "unrelated forbidden")]
#[test_case(500, Some("0"), None, None, "", None; "server error")]
#[test_case(200, Some("0"), None, None, "", None; "last successful request")]
#[test_case(429, None, Some("invalid"), None, "", Some(60); "malformed timing")]
#[test_case(429, None, Some("18446744073709551615"), None, "", Some(60); "overflow timing")]
#[test_case(403, Some("0"), Some("18446744073709551615"), Some("1120"), "", Some(120); "overflow retry preserves usable reset")]
#[test_case(403, Some("0"), None, Some("999"), "", Some(60); "past reset")]
fn rate_limit_responses(
    status: u16,
    remaining: Option<&str>,
    retry: Option<&str>,
    reset: Option<&str>,
    body: &str,
    seconds: Option<u64>,
) {
    let mut headers = ureq::http::HeaderMap::new();
    for (name, value) in [
        ("x-ratelimit-remaining", remaining),
        ("retry-after", retry),
        ("x-ratelimit-reset", reset),
    ] {
        if let Some(value) = value {
            headers.insert(name, value.parse().expect("header fixture"));
        }
    }
    assert_eq!(
        rate_limit(
            status,
            &headers,
            body,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1000)
        ),
        seconds.map(Duration::from_secs)
    );
}

#[test]
fn network_and_unexpected_failures_have_distinct_statuses() {
    for error in [
        ureq::Error::HostNotFound,
        ureq::Error::ConnectionFailed,
        ureq::Error::Io(io::Error::from(io::ErrorKind::ConnectionReset)),
    ] {
        assert_eq!(
            failure_status(&io::Error::other(error)),
            Outcome::ConnectionFailed
        );
    }
    assert_eq!(
        failure_status(&io::Error::from(io::ErrorKind::TimedOut)),
        Outcome::ConnectionFailed
    );
    for error in [
        io::Error::other(ureq::Error::StatusCode(503)),
        io::Error::other("invalid release metadata"),
    ] {
        assert_eq!(failure_status(&error), Outcome::Failed);
    }
}

#[test]
fn dismissing_a_rate_limit_does_not_allow_another_request() {
    let until = Instant::now() + Duration::from_secs(60);
    let client = Client {
        shared: Arc::new((
            Mutex::new(Shared {
                automatic: false,
                phase: Phase::Cooldown { until, due: until },
                result: Some(Outcome::RateLimited),
                dismissed: false,
            }),
            Condvar::new(),
        )),
    };
    client.dismiss();
    client.check();
    assert_eq!(
        client.status(),
        State {
            status: Status::RateLimited(Some(until)),
            dismissed: true
        }
    );
    assert!(!matches!(
        client.shared.0.lock().expect("state lock").phase,
        Phase::Requested
    ));
}

#[test]
fn cooldown_expiry_notifies_without_checking_and_preserves_dismissal() {
    let shared = Arc::new((
        Mutex::new(Shared {
            automatic: false,
            phase: Phase::Cooldown {
                until: Instant::now(),
                due: Instant::now(),
            },
            result: Some(Outcome::RateLimited),
            dismissed: true,
        }),
        Condvar::new(),
    ));
    let client = Client {
        shared: Arc::clone(&shared),
    };
    let worker_shared = Arc::clone(&shared);
    let (notified, notifications) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        run(
            &worker_shared,
            &|| {
                notified.send(()).expect("listener");
            },
            &Instant::now,
            || panic!("expiry must not request an update"),
        );
    });
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("expiry notification");
    assert_eq!(
        client.status(),
        State {
            status: Status::RateLimited(None),
            dismissed: true
        }
    );
    assert!(client.status().can_check());
    drop(client);
    worker.join().expect("worker exits");
}

struct Automatic {
    client: Client,
    started: Instant,
    clock: Arc<Mutex<Instant>>,
    checks: std::sync::mpsc::Receiver<Instant>,
    results: std::sync::mpsc::Sender<io::Result<Outcome>>,
    notifications: std::sync::mpsc::Receiver<()>,
}

impl Automatic {
    fn start(automatic: bool) -> Self {
        let started = Instant::now();
        let clock = Arc::new(Mutex::new(started));
        let (checked, checks) = std::sync::mpsc::channel();
        let (results, replies) = std::sync::mpsc::channel();
        let (notified, notifications) = std::sync::mpsc::channel();
        let now = Arc::clone(&clock);
        let check_clock = Arc::clone(&clock);
        let client = Client::start(
            automatic,
            move || {
                let _listener = notified.send(());
            },
            move || *now.lock().expect("clock lock"),
            move || {
                let _listener = checked.send(*check_clock.lock().expect("clock lock"));
                replies
                    .recv()
                    .unwrap_or_else(|_closed| Err(io::Error::other("fixture closed")))
            },
        )
        .expect("update worker starts");
        Self {
            client,
            started,
            clock,
            checks,
            results,
            notifications,
        }
    }

    fn advance(&self, by: Duration) {
        let shared = &self.client.shared;
        let _worker = shared.0.lock().expect("state lock");
        *self.clock.lock().expect("clock lock") += by;
        shared.1.notify_all();
    }

    fn notified(&self) {
        self.notifications
            .recv_timeout(Duration::from_secs(2))
            .expect("worker notification");
    }

    fn expect_check(&self, at: Duration) {
        let checked = self
            .checks
            .recv_timeout(Duration::from_secs(2))
            .expect("a check starts");
        assert_eq!(checked, self.started + at);
        self.notified();
    }

    fn expect_no_check(&self) {
        assert!(
            self.checks
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "no check is due"
        );
    }

    fn finish(&self, result: io::Result<Outcome>) {
        self.results.send(result).expect("check reply");
        self.notified();
    }
}

fn rate_limited(seconds: u64) -> io::Result<Outcome> {
    Err(io::Error::other(RateLimit(Duration::from_secs(seconds))))
}

#[test]
fn an_enabled_preference_checks_at_start_and_again_24_hours_later() {
    let worker = Automatic::start(true);
    worker.expect_check(Duration::ZERO);
    worker.finish(Ok(Outcome::Current));
    worker.advance(Duration::from_hours(23));
    worker.expect_no_check();
    worker.advance(Duration::from_hours(1));
    worker.expect_check(DAILY);
}

#[test]
fn a_manual_check_restarts_the_automatic_interval() {
    let worker = Automatic::start(true);
    worker.expect_check(Duration::ZERO);
    worker.finish(Ok(Outcome::Current));
    worker.advance(Duration::from_hours(12));
    worker.client.check();
    worker.expect_check(Duration::from_hours(12));
    worker.finish(Ok(Outcome::Current));
    worker.advance(Duration::from_hours(12));
    worker.expect_no_check();
    worker.advance(Duration::from_hours(12));
    worker.expect_check(Duration::from_hours(36));
}

#[test]
fn disabling_the_preference_stops_future_checks_and_lets_the_in_flight_check_finish() {
    let worker = Automatic::start(true);
    worker.expect_check(Duration::ZERO);
    worker.client.configure(false);
    worker.finish(Ok(Outcome::Current));
    assert_eq!(worker.client.status().status, Status::Current);
    worker.advance(DAILY * 2);
    worker.expect_no_check();
}

#[test]
fn cooldown_expiry_adds_no_check_and_the_next_automatic_check_keeps_its_schedule() {
    let worker = Automatic::start(true);
    worker.expect_check(Duration::ZERO);
    worker.finish(rate_limited(60));
    assert_eq!(
        worker.client.status().status,
        Status::RateLimited(Some(worker.started + Duration::from_secs(60)))
    );
    worker.advance(Duration::from_secs(60));
    worker.notified();
    assert_eq!(worker.client.status().status, Status::RateLimited(None));
    worker.expect_no_check();
    worker.advance(Duration::from_mins(23 * 60 + 59));
    worker.expect_check(DAILY);
}

#[test]
fn a_rate_limit_blocks_the_due_automatic_check_until_its_cooldown_ends() {
    let worker = Automatic::start(true);
    worker.expect_check(Duration::ZERO);
    worker.finish(rate_limited(25 * 60 * 60));
    worker.advance(DAILY);
    worker.expect_no_check();
    worker.advance(Duration::from_hours(1));
    worker.notified();
    worker.expect_check(Duration::from_hours(25));
    worker.finish(Ok(Outcome::Current));
    worker.expect_no_check();
}

#[test]
fn each_update_check_state_change_writes_one_transition_record() {
    let log = crate::log::capture(tracing::Level::DEBUG, || {
        let worker = Automatic::start(false);
        worker.client.check();
        worker.expect_check(Duration::ZERO);
        worker.finish(Ok(Outcome::Current));
        worker.client.check();
        worker.expect_check(Duration::ZERO);
        worker.finish(rate_limited(60));
        worker.client.dismiss();
        worker.advance(Duration::from_secs(60));
        worker.notified();
    });
    let transitions: Vec<_> = log
        .iter()
        .filter(|record| record.contains("state transition"))
        .collect();
    assert_eq!(
        transitions,
        [
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Start" to="No result""#,
            r#"DEBUG circadia_app::log: state transition activity="update check" from="No result" to="Checking""#,
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Checking" to="Result available""#,
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Result available" to="Checking""#,
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Checking" to="Rate-limit cooldown""#,
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Rate-limit cooldown" to="Result available""#,
        ]
    );
}
