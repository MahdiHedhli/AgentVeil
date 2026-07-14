#![allow(clippy::expect_used)]

use std::process::Command;

#[test]
fn offline_demo_check_is_value_free_and_succeeds() {
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_agentveil"))
            .args(["demo", "--check"])
            .env_clear()
            .output()
            .expect("offline demo should launch");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
        assert_eq!(
            stdout,
            "demo-check: status=pass route=synthetic_loopback allow=pass tokenize=pass zero_connect=pass tool_reentry=pass dashboard=pass audit=pass output=value_free\n"
        );
        assert!(stderr.is_empty());
        for forbidden in ["@", "[AV_", "Bearer ", "sk-proj-", "10.24."] {
            assert!(!stdout.contains(forbidden));
            assert!(!stderr.contains(forbidden));
        }
    }
}
