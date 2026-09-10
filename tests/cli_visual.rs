mod common;
use common::Sandbox;

#[test]
fn redirected_list_is_stable_tsv_and_color_policy_is_explicit() {
    let sandbox = Sandbox::new();
    sandbox.init();
    sandbox.ok(&["new", "zeta"]);
    sandbox.ok(&["new", "alpha"]);

    let plain = sandbox.output_with_env(&["list"], &[]);
    assert!(plain.status.success());
    assert_eq!(
        String::from_utf8(plain.stdout).unwrap(),
        "active\talpha\t0/2\t0m\nactive\tzeta\t0/2\t0m\n"
    );

    let forced = sandbox.output_with_env(&["list", "--color", "always"], &[]);
    let forced = String::from_utf8(forced.stdout).unwrap();
    assert!(forced.contains("\u{1b}["));
    assert!(!forced.contains("STATE"));
    assert!(forced.find("\talpha").unwrap() < forced.find("\tzeta").unwrap());

    let never = sandbox.output_with_env(&["--color", "never", "list"], &[]);
    assert!(!String::from_utf8(never.stdout).unwrap().contains("\u{1b}["));
    let dumb = sandbox.output_with_env(&["--color", "auto", "list"], &[("TERM", "dumb")]);
    assert!(!String::from_utf8(dumb.stdout).unwrap().contains("\u{1b}["));
    let no_color = sandbox.output_with_env(&["--color", "auto", "list"], &[("NO_COLOR", "1")]);
    assert!(
        !String::from_utf8(no_color.stdout)
            .unwrap()
            .contains("\u{1b}[")
    );
}

#[test]
fn missing_targets_do_not_read_redirected_input() {
    let sandbox = Sandbox::new();
    sandbox.init();

    let new = sandbox.output_with_env(&["--interactive", "new"], &[]);
    assert_eq!(new.status.code(), Some(2));
    assert!(String::from_utf8(new.stderr).unwrap().contains("required"));

    let check = sandbox.output_with_env(&["check"], &[]);
    assert_eq!(check.status.code(), Some(2));
    assert!(
        String::from_utf8(check.stderr)
            .unwrap()
            .contains("required")
    );

    let interactive = sandbox.output_with_env(&["--interactive", "check"], &[]);
    assert_eq!(interactive.status.code(), Some(1));
    assert!(
        String::from_utf8(interactive.stderr)
            .unwrap()
            .contains("attached terminal")
    );
}

#[test]
fn errors_stay_on_stderr_and_destructive_commands_need_no_confirmation() {
    let sandbox = Sandbox::new();
    sandbox.init();
    let missing = sandbox.output_with_env(&["check", "unknown"], &[]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("unknown change ID")
    );

    sandbox.ready("without-yes");
    sandbox.ok(&["complete", "without-yes"]);
    let archived = sandbox.output_with_env(&["archive", "without-yes"], &[]);
    assert!(archived.status.success());
    assert!(
        sandbox
            .path("doco/changes/archived/without-yes/proposal.md")
            .exists()
    );
    assert!(
        !sandbox
            .path("doco/changes/archived/without-yes/work")
            .exists()
    );

    sandbox.ready("with-yes");
    sandbox.ok(&["complete", "with-yes"]);
    let archived = sandbox.output_with_env(&["archive", "with-yes", "--yes"], &[]);
    assert!(archived.status.success());
    assert!(
        sandbox
            .path("doco/changes/archived/with-yes/proposal.md")
            .exists()
    );
}
