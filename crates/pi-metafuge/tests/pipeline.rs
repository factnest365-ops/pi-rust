use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn metafuge() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_metafuge"))
}

fn run(args: &[&str], cwd: &Path) -> Output {
    Command::new(metafuge())
        .args(args)
        .current_dir(cwd)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("metafuge should be runnable")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn forge(out: &Path, cwd: &Path) -> Output {
    run(
        &[
            "forge",
            "--prompt",
            "turn arbitrary text into slugs",
            "--out",
            out.to_str().expect("utf-8 path"),
            "--agent",
            "template",
            "--json",
        ],
        cwd,
    )
}

#[test]
fn forges_a_working_crate_end_to_end() {
    let temp = tempfile::tempdir().expect("temp dir");
    let out = temp.path().join("slugify-crate");
    let output = forge(&out, temp.path());
    assert!(
        output.status.success(),
        "forge failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let report: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout must be exactly one JSON object");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "completed");
    assert_eq!(report["stage"], "done");
    assert_eq!(report["agent"]["kind"], "template");
    assert_eq!(report["agent"]["model_backed"], false);
    assert!(report["sealed_commit"].is_null());
    assert_eq!(
        report["verdict"]["status"], "approved_for_bare_metal",
        "the template run must clear the director"
    );

    let telemetry_dir = out.join(".metafuge").join("telemetry");
    let telemetry_path = telemetry_dir.join("crucible-1.json");
    assert!(telemetry_path.is_file(), "crucible telemetry must exist");
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&telemetry_path).expect("read telemetry"))
            .expect("parse telemetry");
    assert_eq!(record["ok"], true);
    assert_eq!(record["commands"].as_array().map(Vec::len), Some(3));

    let run_record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join(".metafuge/run.json")).expect("read run record"),
    )
    .expect("parse run record");
    assert_eq!(run_record["status"], "completed");
    assert_eq!(run_record["stage"], "done");
    assert!(out.join("src/slug.rs").is_file());
    assert!(out.join("tests/fuzz.rs").is_file());

    let tests = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&out)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("generated cargo test");
    assert!(
        tests.status.success(),
        "the generated crate must pass its own tests\nstdout:\n{}\nstderr:\n{}",
        stdout(&tests),
        stderr(&tests)
    );
}

#[test]
fn resumes_from_the_forge_stage_and_finishes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let out = temp.path().join("resumed-crate");
    let first = forge(&out, temp.path());
    assert!(
        first.status.success(),
        "initial forge failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&first),
        stderr(&first)
    );

    let record_path = out.join(".metafuge").join("run.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record_path).expect("read run record"))
            .expect("parse run record");
    state["stage"] = serde_json::Value::from("forge");
    state["status"] = serde_json::Value::from("running");
    state["round"] = serde_json::Value::from(1);
    state["verdict"] = serde_json::Value::Null;
    state["sealed_commit"] = serde_json::Value::Null;
    for task in state["tasks"].as_array_mut().expect("tasks array") {
        task["status"] = serde_json::Value::from("pending");
        task["attempts"] = serde_json::Value::from(0);
        task["diagnostic"] = serde_json::Value::Null;
    }
    std::fs::write(
        &record_path,
        serde_json::to_string_pretty(&state).expect("encode state"),
    )
    .expect("rewrite run record");
    std::fs::remove_file(out.join("src/slug.rs")).expect("remove generated source");

    let resumed = run(
        &[
            "resume",
            "--out",
            out.to_str().expect("utf-8 path"),
            "--json",
        ],
        temp.path(),
    );
    assert!(
        resumed.status.success(),
        "resume failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&resumed),
        stderr(&resumed)
    );
    let report: serde_json::Value =
        serde_json::from_str(&stdout(&resumed)).expect("resume must emit one JSON object");
    assert_eq!(report["status"], "completed");
    assert_eq!(report["stage"], "done");
    assert!(
        out.join("src/slug.rs").is_file(),
        "resume must re-run the forge stage"
    );
    assert!(
        out.join(".metafuge")
            .join("telemetry")
            .join("crucible-1.json")
            .is_file()
    );
}
