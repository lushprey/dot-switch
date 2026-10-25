//! Integration tests for the switch engine using a temp XDG_CONFIG_HOME.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use tempfile::TempDir;

// Tests in this file mutate process-global env vars (XDG_CONFIG_HOME / XDG_STATE_HOME),
// so they must run serially. The guard returned from setup() holds the lock for the
// entire test body.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn setup() -> (MutexGuard<'static, ()>, TempDir, PathBuf, PathBuf) {
    let guard = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    let xdg_config = tmp.path().join("config");
    let xdg_state = tmp.path().join("state");
    fs::create_dir_all(&xdg_config).unwrap();
    fs::create_dir_all(&xdg_state).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &xdg_config);
    std::env::set_var("XDG_STATE_HOME", &xdg_state);
    (guard, tmp, xdg_config, xdg_state)
}

#[test]
fn test_init_creates_dirs_and_config() {
    let (_guard, _tmp, xdg_config, _xdg_state) = setup();
    let cfg = dot_switch::config::Config::load_or_init().unwrap();
    cfg.validate().unwrap();

    let config_path = dot_switch::config::Config::config_path().unwrap();
    assert!(config_path.exists(), "config.toml should be written on init");

    let profiles_repo = PathBuf::from(&cfg.profiles_repo);
    assert!(profiles_repo.starts_with(&xdg_config));
}

#[test]
fn test_manifest_load_validates_paths() {
    let (_guard, _tmp, _xc, _xs) = setup();
    let tmpdir = TempDir::new().unwrap();
    let manifest_dir = tmpdir.path().join("my-profile");
    fs::create_dir_all(&manifest_dir).unwrap();

    fs::write(
        manifest_dir.join("dot-switch.toml"),
        r#"
name = "bad"
[[files]]
source = "a.conf"
target = "/etc/passwd"
"#,
    )
    .unwrap();
    let res = dot_switch::manifest::Manifest::load(&manifest_dir);
    assert!(res.is_err(), "absolute target should be rejected");

    fs::write(
        manifest_dir.join("dot-switch.toml"),
        r#"
name = "bad"
[[files]]
source = "../escape.conf"
target = "x.conf"
"#,
    )
    .unwrap();
    let res = dot_switch::manifest::Manifest::load(&manifest_dir);
    assert!(res.is_err(), ".. in source should be rejected");
}

#[test]
fn test_apply_creates_symlinks() {
    let (_guard, _tmp, xdg_config, _xdg_state) = setup();

    let profiles_repo = xdg_config.join("dot-switch").join("profiles");
    fs::create_dir_all(profiles_repo.join("dark").join("hypr")).unwrap();
    fs::write(
        profiles_repo.join("dark").join("hypr").join("hyprland.conf"),
        "# dark",
    )
    .unwrap();
    fs::write(
        profiles_repo.join("dark").join("dot-switch.toml"),
        r#"
name = "dark"
[[files]]
source = "hypr/hyprland.conf"
target = "hypr/hyprland.conf"
"#,
    )
    .unwrap();

    let mut cfg = dot_switch::config::Config::default();
    cfg.profiles_repo = profiles_repo.to_string_lossy().into_owned();

    let opts = dot_switch::switch::SwitchOptions::default();
    let report = dot_switch::switch::apply_profile(&cfg, "dark", &opts).unwrap();
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].outcome, dot_switch::switch::Outcome::Created);

    let live = xdg_config.join("hypr").join("hyprland.conf");
    assert!(live.is_symlink(), "target should be a symlink");
    let content = fs::read_to_string(&live).unwrap();
    assert_eq!(content, "# dark");

    let state = dot_switch::state::State::load().unwrap();
    assert_eq!(state.current.as_deref(), Some("dark"));
}

#[test]
fn test_apply_is_idempotent() {
    let (_guard, _tmp, xdg_config, _xdg_state) = setup();
    let profiles_repo = xdg_config.join("dot-switch").join("profiles");
    fs::create_dir_all(profiles_repo.join("p").join("app")).unwrap();
    fs::write(profiles_repo.join("p").join("app").join("c"), "X").unwrap();
    fs::write(
        profiles_repo.join("p").join("dot-switch.toml"),
        r#"
name = "p"
[[files]]
source = "app/c"
target = "app/c"
"#,
    )
    .unwrap();

    let mut cfg = dot_switch::config::Config::default();
    cfg.profiles_repo = profiles_repo.to_string_lossy().into_owned();

    let opts = dot_switch::switch::SwitchOptions::default();
    let _r1 = dot_switch::switch::apply_profile(&cfg, "p", &opts).unwrap();
    let r2 = dot_switch::switch::apply_profile(&cfg, "p", &opts).unwrap();
    assert_eq!(
        r2.files[0].outcome,
        dot_switch::switch::Outcome::AlreadyLinked,
        "second switch to same profile should be a no-op"
    );
}

#[test]
fn test_only_filter_matches_target() {
    let (_guard, _tmp, xdg_config, _xdg_state) = setup();
    let profiles_repo = xdg_config.join("dot-switch").join("profiles");
    fs::create_dir_all(profiles_repo.join("p").join("a")).unwrap();
    fs::create_dir_all(profiles_repo.join("p").join("b")).unwrap();
    fs::write(profiles_repo.join("p").join("a").join("1.conf"), "A").unwrap();
    fs::write(profiles_repo.join("p").join("b").join("2.qml"), "B").unwrap();
    fs::write(
        profiles_repo.join("p").join("dot-switch.toml"),
        r#"
name = "p"
[[files]]
source = "a/1.conf"
target = "a/1.conf"
[[files]]
source = "b/2.qml"
target = "b/2.qml"
"#,
    )
    .unwrap();

    let mut cfg = dot_switch::config::Config::default();
    cfg.profiles_repo = profiles_repo.to_string_lossy().into_owned();

    let opts = dot_switch::switch::SwitchOptions {
        only: Some("b/2.qml".into()),
        ..Default::default()
    };
    let report = dot_switch::switch::apply_profile(&cfg, "p", &opts).unwrap();
    assert_eq!(report.files.len(), 1, "--only should filter to 1 file");
    assert_eq!(report.files[0].target, "b/2.qml");
}

#[test]
fn test_dry_run_does_not_write() {
    let (_guard, _tmp, xdg_config, _xdg_state) = setup();
    let profiles_repo = xdg_config.join("dot-switch").join("profiles");
    fs::create_dir_all(profiles_repo.join("p").join("app")).unwrap();
    fs::write(profiles_repo.join("p").join("app").join("c"), "X").unwrap();
    fs::write(
        profiles_repo.join("p").join("dot-switch.toml"),
        r#"
name = "p"
[[files]]
source = "app/c"
target = "app/c"
"#,
    )
    .unwrap();

    let mut cfg = dot_switch::config::Config::default();
    cfg.profiles_repo = profiles_repo.to_string_lossy().into_owned();

    let opts = dot_switch::switch::SwitchOptions {
        dry_run: true,
        ..Default::default()
    };
    let report = dot_switch::switch::apply_profile(&cfg, "p", &opts).unwrap();
    assert_eq!(report.files[0].outcome, dot_switch::switch::Outcome::WouldCreate);

    let live = xdg_config.join("app").join("c");
    assert!(!live.exists(), "dry-run must not create the symlink");

    let state = dot_switch::state::State::load().unwrap();
    assert!(state.current.is_none());
}
