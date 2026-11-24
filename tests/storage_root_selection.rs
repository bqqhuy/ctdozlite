//! Exercise production path selection in child processes with isolated user directories.
use std::path::{Path, PathBuf};
use std::process::Command;

fn doctor_root(executable: &Path, user_data: &Path, args: &[&str]) -> PathBuf {
    let output = Command::new(executable)
        .arg("--doctor")
        .args(args)
        .env_remove("CTDOZ_DATA_DIR")
        .env_remove("CTDOZ_ALLOW_TEST_LOOPBACK")
        .env_remove("RUST_TEST_THREADS")
        .env("LOCALAPPDATA", user_data)
        .env("HOME", user_data)
        .env("XDG_DATA_HOME", user_data)
        .output()
        .unwrap();
    assert!(output.status.success(), "doctor failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    PathBuf::from(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("Data directory: "))
            .expect("doctor must report its data directory"),
    )
}

#[test]
fn login_storage_stays_stable_when_executable_or_profile_contains_test() {
    let isolated = tempfile::tempdir().unwrap();
    let user_data = isolated.path().join("test-user");
    std::fs::create_dir_all(&user_data).unwrap();
    #[cfg(target_os = "macos")]
    let expected = user_data.join("Library/Application Support/ctdozlite");
    #[cfg(not(target_os = "macos"))]
    let expected = user_data.join("ctdozlite");

    for (directory, filename) in [
        ("ordinary", "ZaloLite-v0.016-arm-test-0d62e22"),
        ("test-build", "ctdozlite"),
        ("ordinary", "ctdozlite"),
    ] {
        let directory = isolated.path().join(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join(format!("{filename}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(env!("CARGO_BIN_EXE_ctdozlite"), &executable).unwrap();
        for args in [vec![], vec!["--profile=latest"]] {
            let expected = if args.is_empty() {
                expected.clone()
            } else {
                expected.join("profiles/latest")
            };
            assert_eq!(doctor_root(&executable, &user_data, &args), expected);
            assert_eq!(doctor_root(&executable, &user_data, &args), expected);
        }
    }
}
