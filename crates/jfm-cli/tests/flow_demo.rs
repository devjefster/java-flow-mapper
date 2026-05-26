use std::path::Path;

use assert_cmd::Command;

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
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-api/demo");
    let mut command = Command::cargo_bin("jfm").expect("binary exists");
    command.arg("flow").arg(demo).arg(endpoint);
    command.args(extra_args);

    let output = command.assert().success().get_output().stdout.clone();
    String::from_utf8(output).expect("stdout is utf8")
}
