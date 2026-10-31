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

#[test_case("v0.1.0", &Status::Current; "equal")]
#[test_case("v0.0.9", &Status::Current; "installed newer")]
#[test_case("v0.10.0", &Status::Available("https://github.com/kvnxiao/circadia/releases/tag/v0.10.0".into()); "numeric version comparison")]
#[test_case("v0.2.0-preview", &Status::Unpublished; "prerelease tag")]
fn stable_version_comparison(tag: &str, expected: &Status) {
    assert_eq!(
        stable(Some(&release(tag)), "0.1.0").expect("valid versions"),
        *expected
    );
}

#[test]
fn missing_release_and_incomplete_assets_do_not_claim_current_or_offer_downloads() {
    assert_eq!(
        stable(None, "0.1.0").expect("no release"),
        Status::Unpublished
    );
    let mut latest = release("v0.2.0");
    latest.assets.pop();
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("missing checksum"),
        Status::Unpublished
    );
    latest.assets.clear();
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("missing installer"),
        Status::Unpublished
    );
}

#[test]
fn draft_and_prerelease_flags_exclude_releases() {
    let mut latest = release("v0.2.0");
    latest.draft = true;
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("draft"),
        Status::Unpublished
    );
    latest.draft = false;
    latest.prerelease = true;
    assert_eq!(
        stable(Some(&latest), "0.1.0").expect("prerelease"),
        Status::Unpublished
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
        Status::Available("https://github.com/kvnxiao/circadia/actions/runs/100".into())
    );
}

#[test]
fn preview_does_not_offer_expired_deleted_or_malformed_artifacts() {
    assert_eq!(
        preview_packages(runs(), |id| Ok(Some(artifact(id, true))), ahead)
            .expect("expired packages"),
        Status::Unpublished
    );
    assert_eq!(
        preview_packages(runs(), |_| Ok(None), ahead).expect("deleted packages"),
        Status::Unpublished
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
    assert_eq!(result, Status::Unpublished);
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
    assert_eq!(result, Status::Current);
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
    assert_eq!(result, Status::Failed);
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
        Status::Available("https://github.com/kvnxiao/circadia/actions/runs/110".into())
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
        Status::Unpublished
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
        Mutex::new(Shared {
            automatic: false,
            requested: false,
            stopped: false,
            status: Status::Idle,
        }),
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
            || {
                finishes.recv().expect("controlled check completion");
                Ok(Status::Current)
            },
        );
    });
    client.check();
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("checking notification");
    client.check();
    assert!(!shared.0.lock().expect("state lock").requested);
    assert_eq!(client.status(), Status::Checking);
    drop(client);
    finish.send(()).expect("release network fixture");
    worker.join().expect("worker exits");
    assert!(notifications.try_recv().is_err());
    assert_eq!(
        shared.0.lock().expect("state lock").status,
        Status::Checking
    );
}
