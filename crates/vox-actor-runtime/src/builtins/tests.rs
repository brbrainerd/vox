use super::*;

#[test]
fn fast_hash_is_deterministic() {
    assert_eq!(vox_hash_fast("hello world"), vox_hash_fast("hello world"));
    assert_eq!(vox_hash_fast("hello world").len(), 32);
}

#[test]
fn fast_hash_differs_for_different_inputs() {
    assert_ne!(vox_hash_fast("foo"), vox_hash_fast("bar"));
}

#[test]
fn list_dir_finds_file() {
    let dir = std::env::temp_dir().join(format!("vox-list-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.txt"), b"a").unwrap();
    let res = vox_list_dir(dir.to_string_lossy().as_ref()).unwrap();
    assert!(res.iter().any(|n| n == "x.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_detailed_includes_file_metadata() {
    let dir = std::env::temp_dir().join(format!("vox-list-detailed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("y.txt"), b"zz").unwrap();
    let rows = vox_fs_list_dir_detailed(dir.to_string_lossy().as_ref()).unwrap();
    let y = rows.iter().find(|r| r.name == "y.txt").expect("y.txt row");
    assert!(y.is_file);
    assert_eq!(y.size, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn csv_parse_records_reads_header() {
    let v = vox_csv_parse_records("a,b\n1,2\n").unwrap();
    let arr = v.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    let o = arr[0].as_object().expect("obj");
    assert_eq!(o.get("a").and_then(|x| x.as_str()), Some("1"));
}

#[test]
fn process_run_capture_lines_echo() {
    let lines = if cfg!(windows) {
        vox_process_run_capture_lines("cmd.exe", &["/C".into(), "echo".into(), "hi".into()])
    } else {
        vox_process_run_capture_lines("echo", &["hi".into()])
    }
    .expect("echo");
    assert!(lines.iter().any(|l| l.contains("hi")), "{lines:?}");
}

#[test]
fn fast_hash_differs_for_similar_inputs() {
    // Avalanche effect: single char change → totally different hash
    assert_ne!(vox_hash_fast("gain"), vox_hash_fast("Gain"));
    assert_ne!(vox_hash_fast("loss"), vox_hash_fast("los"));
}

#[test]
fn secure_hash_is_deterministic() {
    assert_eq!(
        vox_hash_secure("hello world"),
        vox_hash_secure("hello world")
    );
    assert_eq!(vox_hash_secure("hello world").len(), 64);
}

#[tokio::test]
async fn openclaw_gateway_call_invalid_json_is_reported_without_adapter() {
    let mut adapter = None;
    let err = handle_openclaw_op(
        &mut adapter,
        OpenClawOp::GatewayCall {
            method: "subscriptions.list".to_string(),
            params_json: "{not-valid-json".to_string(),
        },
    )
    .await
    .expect_err("invalid JSON must fail before adapter access");
    assert!(
        err.contains("invalid params_json"),
        "unexpected error: {err}"
    );
}

#[test]
fn openclaw_worker_send_failure_is_reported() {
    let (tx, rx) = std::sync::mpsc::channel::<OpenClawRequest>();
    drop(rx);
    let worker = OpenClawWorker { tx };
    let err = run_openclaw_op_with_worker(&worker, OpenClawOp::ListSkills)
        .expect_err("send should fail when receiver is dropped");
    assert!(
        err.contains("openclaw worker send failed"),
        "unexpected error: {err}"
    );
}

#[test]
fn openclaw_worker_recv_failure_is_reported() {
    let (tx, rx) = std::sync::mpsc::channel::<OpenClawRequest>();
    std::thread::spawn(move || {
        if let Ok(req) = rx.recv() {
            drop(req.reply_tx);
        }
    });
    let worker = OpenClawWorker { tx };
    let err = run_openclaw_op_with_worker(&worker, OpenClawOp::ListSkills)
        .expect_err("recv should fail when worker closes reply channel");
    assert!(
        err.contains("openclaw worker recv failed"),
        "unexpected error: {err}"
    );
}

#[test]
fn secure_hash_known_vector() {
    // BLAKE3 test vector from official spec
    let h = vox_hash_secure("");
    assert_eq!(
        h,
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
}

#[test]
fn secure_hash_differs_from_fast_hash() {
    let input = "test input";
    assert_ne!(vox_hash_fast(input), vox_hash_secure(input));
}

#[test]
fn uuid_is_unique() {
    let u1 = vox_uuid();
    let u2 = vox_uuid();
    assert_ne!(u1, u2);
    assert!(u1.starts_with("vox-"));
    // Format: vox-{16 hex}-{16 hex}
    let parts: Vec<&str> = u1.splitn(3, '-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[1].len(), 16);
    assert_eq!(parts[2].len(), 16);
}

#[test]
fn uuid_counter_is_monotonic() {
    let ids: Vec<String> = (0..100).map(|_| vox_uuid()).collect();
    // All must be unique
    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), 100);
}

#[test]
fn now_ms_is_reasonable() {
    let ts = vox_now_ms();
    // Must be after 2025-01-01T00:00:00Z (1735689600000 ms)
    assert!(ts > 1_735_689_600_000, "timestamp too old: {}", ts);
}

#[test]
fn process_run_capture_reads_echo() {
    let cap = if cfg!(windows) {
        vox_process_run_capture("cmd.exe", &["/C".into(), "echo".into(), "hello".into()])
    } else {
        vox_process_run_capture("echo", &["hello".into()])
    }
    .expect("spawn echo");
    assert_eq!(cap.exit, 0);
    assert!(cap.stdout.contains("hello"), "stdout={:?}", cap.stdout);
}

#[test]
fn process_which_finds_system_executable() {
    let name = if cfg!(windows) { "cmd.exe" } else { "sh" };
    let resolved = vox_process_which(name);
    assert!(
        resolved.is_some(),
        "expected to resolve {name} on PATH, got None"
    );
    let p = resolved.unwrap();
    assert!(!p.trim().is_empty(), "empty path for {name}");
}

#[test]
fn fs_glob_finds_temp_file() {
    let dir = std::env::temp_dir().join(format!("vox-glob-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), b"x").unwrap();
    let pat = dir.join("*.txt").to_string_lossy().into_owned();
    let got = vox_fs_glob(&pat).unwrap();
    assert!(
        got.iter().any(|p| p.ends_with("a.txt")),
        "glob {pat} -> {got:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_join_many_joins_segments() {
    let segs = vec!["a".into(), "b".into(), "c".into()];
    let j = vox_path_join_many(&segs);
    assert!(j.contains("a") && j.contains("b") && j.contains("c"));
    assert_eq!(vox_path_join_many(&[]), ".");
}

#[test]
fn json_read_str_and_f64() {
    let raw = r#"{"name":"x","n":3,"f":1.5}"#;
    assert_eq!(vox_json_read_str(raw, "name").unwrap(), "x");
    assert!((vox_json_read_f64(raw, "n").unwrap() - 3.0).abs() < f64::EPSILON);
    assert!((vox_json_read_f64(raw, "f").unwrap() - 1.5).abs() < f64::EPSILON);
}

#[tokio::test]
async fn http_invalid_json_body_is_rejected_before_network() {
    let client = vox_http_client::client();
    let err = handle_http_op(
        &client,
        HttpOp::PostJson {
            url: "http://127.0.0.1:1".to_string(),
            body_json: "{not-json".to_string(),
        },
    )
    .await
    .expect_err("invalid JSON body should fail before HTTP call");
    assert!(err.contains("invalid JSON body"), "unexpected error: {err}");
}

#[tokio::test]
async fn http_invalid_url_reports_error() {
    let client = vox_http_client::client();
    let err = handle_http_op(
        &client,
        HttpOp::GetText {
            url: "notaurl".to_string(),
        },
    )
    .await
    .expect_err("invalid URL must fail");
    assert!(!err.trim().is_empty(), "error should not be empty");
}

#[test]
fn http_worker_send_failure_is_reported() {
    let (tx, rx) = std::sync::mpsc::channel::<HttpRequest>();
    drop(rx);
    let worker = HttpWorker { tx };
    let err = run_http_op_with_worker(
        &worker,
        HttpOp::GetText {
            url: "https://example.invalid".to_string(),
        },
    )
    .expect_err("send should fail when receiver is dropped");
    assert!(
        err.contains("http worker send failed"),
        "unexpected error: {err}"
    );
}

#[test]
fn http_worker_recv_failure_is_reported() {
    let (tx, rx) = std::sync::mpsc::channel::<HttpRequest>();
    std::thread::spawn(move || {
        if let Ok(req) = rx.recv() {
            drop(req.reply_tx);
        }
    });
    let worker = HttpWorker { tx };
    let err = run_http_op_with_worker(
        &worker,
        HttpOp::GetText {
            url: "https://example.invalid".to_string(),
        },
    )
    .expect_err("recv should fail when worker closes reply channel");
    assert!(
        err.contains("http worker recv failed"),
        "unexpected error: {err}"
    );
}

#[test]
fn process_run_capture_ex_respects_cwd() {
    let dir = std::env::temp_dir().join(format!("vox-cwd-cap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("marker.txt"), b"ok").unwrap();
    let cap = if cfg!(windows) {
        vox_process_run_capture_ex(
            "cmd.exe",
            &["/C".into(), "type".into(), "marker.txt".into()],
            &dir.to_string_lossy(),
            &[],
        )
    } else {
        vox_process_run_capture_ex("cat", &["marker.txt".into()], &dir.to_string_lossy(), &[])
    }
    .unwrap();
    assert_eq!(cap.exit, 0);
    assert!(cap.stdout.contains("ok"));
    let _ = std::fs::remove_dir_all(&dir);
}
