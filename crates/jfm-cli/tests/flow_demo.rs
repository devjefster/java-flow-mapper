use std::path::Path;

use assert_cmd::Command;
use jfm_graph::SurrealGraphStore;
use tempfile::tempdir;

#[test]
fn index_demo_caches_project_index() {
    let demo = demo_root();
    let tmp = tempdir().expect("tempdir");
    let graph_dir = tmp.path().join("graph");

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .arg("index")
        .arg(&demo)
        .arg("--graph-dir")
        .arg(&graph_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");

    assert!(stdout.contains("Indexed "));
    assert!(stdout.contains(" classes, "));
    assert!(stdout.contains(" endpoints into "));
    assert!(stdout.contains(&graph_dir.display().to_string()));

    let store = SurrealGraphStore::open(&graph_dir).expect("graph opens");
    let index = store
        .load_project_index()
        .expect("index loads")
        .expect("index cached");
    assert!(!index.classes.is_empty());
    assert!(!index.endpoints.is_empty());
}

#[test]
fn index_defaults_root_to_current_directory() {
    let demo = demo_root();
    let tmp = tempdir().expect("tempdir");
    let graph_dir = tmp.path().join("graph");

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .current_dir(&demo)
        .arg("index")
        .arg("--graph-dir")
        .arg(&graph_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");

    assert!(stdout.contains("Indexed "));
    let store = SurrealGraphStore::open(&graph_dir).expect("graph opens");
    let index = store
        .load_project_index()
        .expect("index loads")
        .expect("index cached");
    assert!(!index.endpoints.is_empty());
}

#[test]
fn entrypoints_markdown_lists_cached_endpoints() {
    let graph_dir = index_demo_to_temp_graph();

    let stdout = run_entrypoints(graph_dir.path(), &[]);

    assert!(stdout.starts_with("# Entry Points\n\n"));
    assert!(
        stdout.contains("- `GET /users` -> `com.example.demo.controller.UserController#findAll()`")
    );
    assert!(stdout.contains(
        "- `GET /users/{id}` -> `com.example.demo.controller.UserController#findById(Long)`"
    ));
    assert!(stdout.contains(
        "- `POST /users` -> `com.example.demo.controller.UserController#create(CreateUserRequest)`"
    ));
}

#[test]
fn entrypoints_json_filters_method_and_path_prefix() {
    let graph_dir = index_demo_to_temp_graph();

    let stdout = run_entrypoints(
        graph_dir.path(),
        &[
            "--method",
            "GET",
            "--path-prefix",
            "/users/active",
            "--format",
            "json",
        ],
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let endpoints = value.as_array().expect("json array");

    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0]["method"], "GET");
    assert_eq!(endpoints[0]["path"], "/users/active");
    assert_eq!(
        endpoints[0]["handler"],
        "com.example.demo.controller.UserController#findActive()"
    );
}

#[test]
fn entrypoints_missing_cache_tells_user_to_index_first() {
    let tmp = tempdir().expect("tempdir");
    let graph_dir = tmp.path().join("missing-graph");
    let missing_root = tmp.path().join("missing-root");

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .arg("entrypoints")
        .arg(&missing_root)
        .arg("--graph-dir")
        .arg(&graph_dir)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).expect("stderr is utf8");

    assert!(stderr.contains("no cached project index found at "));
    assert!(stderr.contains("Run `jfm index "));
    assert!(stderr.contains(" first."));
}

#[test]
fn entrypoints_defaults_root_to_current_directory() {
    let demo = demo_root();
    let graph_dir = index_demo_to_temp_graph();

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .current_dir(demo)
        .arg("entrypoints")
        .arg("--graph-dir")
        .arg(graph_dir.path())
        .arg("--method")
        .arg("GET")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");

    assert!(stdout.contains("`GET /users/{id}`"));
    assert!(!stdout.contains("`POST /users`"));
}

#[test]
fn doctor_markdown_reports_cached_project_health() {
    let graph_dir = index_demo_to_temp_graph();

    let stdout = run_doctor(graph_dir.path(), &[]);

    assert!(stdout.starts_with("# JFM Doctor\n\n"));
    assert!(stdout.contains("- Classes: "));
    assert!(stdout.contains("- Methods: "));
    assert!(stdout.contains("- Endpoints: "));
    assert!(stdout.contains("- Flows built: "));
    assert!(stdout.contains("## Confidence Totals"));
    assert!(stdout.contains(
        "- `GET /users/{id}` -> `com.example.demo.controller.UserController#findById(Long)`: ok"
    ));
}

#[test]
fn doctor_json_reports_cached_project_health() {
    let graph_dir = index_demo_to_temp_graph();

    let stdout = run_doctor(graph_dir.path(), &["--format", "json"]);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");

    assert!(value["summary"]["classes"].as_u64().expect("classes") > 0);
    assert!(value["summary"]["methods"].as_u64().expect("methods") > 0);
    assert!(value["summary"]["endpoints"].as_u64().expect("endpoints") > 0);
    assert_eq!(
        value["summary"]["endpoints"],
        value["summary"]["flows_built"]
    );
    assert!(value["confidence"]["resolved"].as_u64().expect("resolved") > 0);
    assert!(
        value["endpoints"]
            .as_array()
            .expect("endpoints")
            .iter()
            .any(|endpoint| endpoint["method"] == "GET" && endpoint["path"] == "/users/{id}")
    );
}

#[test]
fn doctor_missing_cache_tells_user_to_index_first() {
    let tmp = tempdir().expect("tempdir");
    let graph_dir = tmp.path().join("missing-graph");
    let missing_root = tmp.path().join("missing-root");

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .arg("doctor")
        .arg(&missing_root)
        .arg("--graph-dir")
        .arg(&graph_dir)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).expect("stderr is utf8");

    assert!(stderr.contains("no cached project index found at "));
    assert!(stderr.contains("Run `jfm index "));
    assert!(stderr.contains(" first."));
}

#[test]
fn doctor_defaults_root_to_current_directory() {
    let demo = demo_root();
    let graph_dir = index_demo_to_temp_graph();

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .current_dir(demo)
        .arg("doctor")
        .arg("--graph-dir")
        .arg(graph_dir.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");

    assert!(value["summary"]["endpoints"].as_u64().expect("endpoints") > 0);
}

#[test]
fn flow_defaults_root_to_current_directory() {
    let demo = demo_root();

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .current_dir(demo)
        .arg("flow")
        .arg("GET /users/{id}")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");

    assert!(stdout.contains("# GET /users/{id}"));
    assert!(stdout.contains("UserController#findById(Long)"));
}

#[test]
fn flow_uses_cached_index_when_graph_dir_is_available() {
    let graph_dir = index_demo_to_temp_graph();
    let missing_root = graph_dir.path().join("missing-root");

    let output = Command::cargo_bin("jfm")
        .expect("binary exists")
        .arg("flow")
        .arg(&missing_root)
        .arg("GET /users/{id}")
        .arg("--graph-dir")
        .arg(graph_dir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");

    assert!(stdout.contains("# GET /users/{id}"));
    assert!(stdout.contains("UserController#findById(Long)"));
}

#[test]
fn flow_get_users_by_id_renders_expected_markdown() {
    let stdout = run_flow("GET /users/{id}", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_by_id_renders_expected_json() {
    let stdout = run_flow("GET /users/{id}", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_by_id_renders_expected_mermaid() {
    let stdout = run_flow("GET /users/{id}", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_by_id_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "GET /users/{id}",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_post_users_renders_expected_markdown() {
    let stdout = run_flow("POST /users", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_post_users_renders_expected_json() {
    let stdout = run_flow("POST /users", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_post_users_renders_expected_mermaid() {
    let stdout = run_flow("POST /users", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_post_users_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "POST /users",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_renders_expected_markdown() {
    let stdout = run_flow("GET /users", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_renders_expected_json() {
    let stdout = run_flow("GET /users", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_renders_expected_mermaid() {
    let stdout = run_flow("GET /users", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "GET /users",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_put_users_by_id_renders_expected_markdown() {
    let stdout = run_flow("PUT /users/{id}", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_put_users_by_id_renders_expected_json() {
    let stdout = run_flow("PUT /users/{id}", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_put_users_by_id_renders_expected_mermaid() {
    let stdout = run_flow("PUT /users/{id}", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_put_users_by_id_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "PUT /users/{id}",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_delete_users_by_id_renders_expected_markdown() {
    let stdout = run_flow("DELETE /users/{id}", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_delete_users_by_id_renders_expected_json() {
    let stdout = run_flow("DELETE /users/{id}", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_delete_users_by_id_renders_expected_mermaid() {
    let stdout = run_flow("DELETE /users/{id}", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_delete_users_by_id_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "DELETE /users/{id}",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_by_id_with_max_depth_2_renders_truncated_markdown() {
    let stdout = run_flow("GET /users/{id}", &["--max-depth", "2"]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_by_id_with_max_depth_2_renders_truncated_json() {
    let stdout = run_flow("GET /users/{id}", &["--format", "json", "--max-depth", "2"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_by_id_with_max_depth_2_renders_truncated_mermaid() {
    let stdout = run_flow(
        "GET /users/{id}",
        &["--format", "mermaid", "--max-depth", "2"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_active_renders_expected_markdown() {
    let stdout = run_flow("GET /users/active", &[]);

    insta::with_settings!({
        filters => vec![
            (r"`[^`]*demo-api/demo/src/main/java", "`demo-api/demo/src/main/java"),
            (r":\d+`", ":LINE`"),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_active_renders_expected_json() {
    let stdout = run_flow("GET /users/active", &["--format", "json"]);

    insta::with_settings!({
        filters => vec![
            (r#""file": "[^"]*demo-api/demo/src/main/java"#, r#""file": "demo-api/demo/src/main/java"#),
            (r#""line": \d+"#, r#""line": LINE"#),
        ],
    }, {
        insta::assert_snapshot!(stdout);
    });
}

#[test]
fn flow_get_users_active_renders_expected_mermaid() {
    let stdout = run_flow("GET /users/active", &["--format", "mermaid"]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_active_renders_expected_mermaid_flowchart() {
    let stdout = run_flow(
        "GET /users/active",
        &["--format", "mermaid", "--diagram", "flowchart"],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn flow_get_users_active_with_max_depth_2_renders_truncated_mermaid_flowchart() {
    let stdout = run_flow(
        "GET /users/active",
        &[
            "--format",
            "mermaid",
            "--diagram",
            "flowchart",
            "--max-depth",
            "2",
        ],
    );

    insta::assert_snapshot!(stdout);
}

fn run_flow(endpoint: &str, extra_args: &[&str]) -> String {
    let demo = demo_root();
    let mut command = Command::cargo_bin("jfm").expect("binary exists");
    command.arg("flow").arg(demo).arg(endpoint);
    command.args(extra_args);

    let output = command.assert().success().get_output().stdout.clone();
    String::from_utf8(output).expect("stdout is utf8")
}

fn run_entrypoints(graph_dir: &Path, extra_args: &[&str]) -> String {
    let demo = demo_root();
    let mut command = Command::cargo_bin("jfm").expect("binary exists");
    command
        .arg("entrypoints")
        .arg(demo)
        .arg("--graph-dir")
        .arg(graph_dir);
    command.args(extra_args);

    let output = command.assert().success().get_output().stdout.clone();
    String::from_utf8(output).expect("stdout is utf8")
}

fn run_doctor(graph_dir: &Path, extra_args: &[&str]) -> String {
    let demo = demo_root();
    let mut command = Command::cargo_bin("jfm").expect("binary exists");
    command
        .arg("doctor")
        .arg(demo)
        .arg("--graph-dir")
        .arg(graph_dir);
    command.args(extra_args);

    let output = command.assert().success().get_output().stdout.clone();
    String::from_utf8(output).expect("stdout is utf8")
}

fn index_demo_to_temp_graph() -> tempfile::TempDir {
    let demo = demo_root();
    let tmp = tempdir().expect("tempdir");

    Command::cargo_bin("jfm")
        .expect("binary exists")
        .arg("index")
        .arg(demo)
        .arg("--graph-dir")
        .arg(tmp.path())
        .assert()
        .success();

    tmp
}

fn demo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-api/demo")
}
