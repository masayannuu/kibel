use std::process::Command;

fn assert_help_ok(args: &[&str]) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("kibel"));
    command.args(args).arg("--help");

    let output = command.output().expect("failed to run kibel --help");
    assert_eq!(
        output.status.code(),
        Some(0),
        "help should succeed for command path: {args:?}"
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    assert!(
        stdout.contains("Usage:"),
        "help output must include Usage header for command path: {args:?}"
    );
}

#[test]
fn all_command_paths_expose_help() {
    let command_paths: &[&[&str]] = &[
        &[],
        &["auth"],
        &["auth", "login"],
        &["auth", "logout"],
        &["auth", "status"],
        &["config"],
        &["config", "set"],
        &["config", "set", "team"],
        &["config", "profiles"],
        &["search"],
        &["search", "note"],
        &["search", "folder"],
        &["group"],
        &["group", "list"],
        &["folder"],
        &["folder", "list"],
        &["folder", "get"],
        &["folder", "get-from-path"],
        &["folder", "notes"],
        &["folder", "create"],
        &["feed"],
        &["feed", "sections"],
        &["comment"],
        &["comment", "create"],
        &["comment", "reply"],
        &["note"],
        &["note", "create"],
        &["note", "get"],
        &["note", "get-from-path"],
        &["note", "move-to-folder"],
        &["note", "attach-to-folder"],
        &["note", "update"],
        &["graphql"],
        &["graphql", "run"],
        &["completion"],
        &["version"],
    ];

    for args in command_paths {
        assert_help_ok(args);
    }
}
