//! dot-switch entry point.

use std::path::PathBuf;
use std::process::Command as StdCommand;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};
use clap_complete::generate;

use dot_switch::cli::{Cli, Command, ConfigAction};
use dot_switch::config::Config;
use dot_switch::manifest;
use dot_switch::state;
use dot_switch::switch;
use dot_switch::util::{ensure_dir, expand_path, init_logging};
use dot_switch::util;
use dot_switch::switch::{SwitchOptions, apply_all, apply_profile, diff_profile, list_profiles, revert, snapshot_paths};

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose > 0);

    match cli.command {
        // No subcommand → launch TUI
        None => run_tui(),

        Some(cmd) => run_command(cmd),
    }
}

fn run_tui() -> Result<()> {
    let cfg = Config::load_or_init()?;
    cfg.validate()?;
    let selected = dot_switch::tui::launch(cfg.clone())?;
    if let Some(name) = selected {
        println!("Switching to `{}`...", name);
        let opts = SwitchOptions::default();
        let report = apply_profile(&cfg, &name, &opts)?;
        println!("{}", report.summary_line());
        for f in &report.files {
            let mark = match f.outcome {
                switch::Outcome::Created => "+",
                switch::Outcome::Replaced => "~",
                switch::Outcome::AlreadyLinked => "=",
                switch::Outcome::WouldCreate | switch::Outcome::WouldReplace => "?",
            };
            println!("  {} {} -> {}", mark, f.source, f.target);
        }
    }
    Ok(())
}

fn run_command(cmd: Command) -> Result<()> {
    match cmd {
        Command::Init => cmd_init(),
        Command::Use { name, only, backup, dry_run, no_reload, reload } => cmd_use(
            &name, only, backup, dry_run, no_reload, reload,
        ),
        Command::Apply { dry_run } => cmd_apply(dry_run),
        Command::List => cmd_list(),
        Command::Diff { name, only } => cmd_diff(&name, only),
        Command::Current => cmd_current(),
        Command::Revert { dry_run } => cmd_revert(dry_run),
        Command::Snapshot { paths } => cmd_snapshot(&paths),
        Command::New { name, from_live } => cmd_new(&name, &from_live),
        Command::Config { action } => cmd_config(action),
        Command::Paths => cmd_paths(),
        Command::Completions { shell } => cmd_completions(shell),
    }
}

fn cmd_init() -> Result<()> {
    let cfg = Config::load_or_init()?;
    // Ensure profiles repo exists
    let repo = expand_path(&cfg.profiles_repo)?;
    ensure_dir(&repo)?;
    // Ensure state dir exists
    let state_dir = Config::state_path()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()).ok_or_else(|| anyhow::anyhow!("no state parent")));
    if let Ok(p) = state_dir {
        ensure_dir(&p)?;
    }
    // Ensure backups dir exists
    ensure_dir(&Config::backups_dir()?)?;
    println!("Initialized dot-switch.");
    println!("  config:    {}", Config::config_path()?.display());
    println!("  profiles:  {}", repo.display());
    println!("  state:     {}", Config::state_path()?.display());
    println!("  backups:   {}", Config::backups_dir()?.display());
    println!();
    println!("Edit `config.toml` to enable reload presets, then run `dot-switch new <name> --from-live <paths>` to capture your first profile.");
    Ok(())
}

fn cmd_use(
    name: &str,
    only: Option<String>,
    backup: bool,
    dry_run: bool,
    no_reload: bool,
    force_reload: bool,
) -> Result<()> {
    let cfg = Config::load_or_init()?;
    cfg.validate()?;
    let opts = SwitchOptions {
        only,
        backup,
        dry_run,
        force_reload,
        no_reload,
    };
    let report = apply_profile(&cfg, name, &opts)?;
    println!("{}", report.summary_line());
    for f in &report.files {
        let mark = match f.outcome {
            switch::Outcome::Created => "+",
            switch::Outcome::Replaced => "~",
            switch::Outcome::AlreadyLinked => "=",
            switch::Outcome::WouldCreate | switch::Outcome::WouldReplace => "?",
        };
        println!("  {} {} -> {}", mark, f.source, f.target);
    }
    Ok(())
}

fn cmd_apply(dry_run: bool) -> Result<()> {
    let cfg = Config::load_or_init()?;
    cfg.validate()?;
    let reports = apply_all(&cfg, dry_run)?;
    for r in reports {
        println!("{}", r.summary_line());
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let cfg = Config::load_or_init()?;
    let repo = PathBuf::from(&cfg.profiles_repo);
    let names = list_profiles(&repo)?;
    if names.is_empty() {
        println!("(no profiles yet — run `dot-switch new <name> --from-live <paths>` to create one)");
        return Ok(());
    }
    let state = state::State::load()?;
    let current = state.current.as_deref();
    for n in &names {
        let marker = if Some(n.as_str()) == current { "*" } else { " " };
        println!("{} {}", marker, n);
    }
    Ok(())
}

fn cmd_diff(name: &str, only: Option<String>) -> Result<()> {
    let cfg = Config::load_or_init()?;
    cfg.validate()?;
    let report = diff_profile(&cfg, name, only.as_deref())?;
    println!("{}", report.summary_line());
    for f in &report.files {
        let mark = match f.outcome {
            switch::Outcome::WouldCreate => "+",
            switch::Outcome::WouldReplace => "~",
            switch::Outcome::AlreadyLinked => "=",
            _ => "?",
        };
        println!("  {} {} -> {}", mark, f.source, f.target);
    }
    Ok(())
}

fn cmd_current() -> Result<()> {
    let state = state::State::load()?;
    match state.current {
        Some(c) => println!("{}", c),
        None => println!("(no active profile)"),
    }
    Ok(())
}

fn cmd_revert(dry_run: bool) -> Result<()> {
    let cfg = Config::load_or_init()?;
    cfg.validate()?;
    match revert(&cfg, dry_run)? {
        Some(report) => {
            println!("Reverted to `{}`.", report.profile);
            println!("{}", report.summary_line());
        }
        None => println!("(no history to revert to)"),
    }
    Ok(())
}

fn cmd_snapshot(paths: &[String]) -> Result<()> {
    let dir = snapshot_paths(paths)?;
    println!("Snapshotted {} file(s) to {}", paths.len(), dir.display());
    Ok(())
}

fn cmd_new(name: &str, from_live: &[String]) -> Result<()> {
    let cfg = Config::load_or_init()?;
    let repo = PathBuf::from(&cfg.profiles_repo);
    let profile_dir = repo.join(name);
    if profile_dir.exists() {
        bail!("profile `{}` already exists at {}", name, profile_dir.display());
    }
    ensure_dir(&profile_dir)?;

    let xdg = switch::xdg_config_home()?;
    let mut manifest_files = Vec::new();

    for live_path in from_live {
        let target_path = xdg.join(live_path);
        if !target_path.exists() && !target_path.is_symlink() {
            bail!("live file `{}` does not exist at {}", live_path, target_path.display());
        }
        // Copy file into profile_dir using same relative path
        let dest = profile_dir.join(live_path);
        if let Some(parent) = dest.parent() {
            ensure_dir(parent)?;
        }
        let meta = std::fs::symlink_metadata(&target_path)?;
        if meta.file_type().is_symlink() {
            // Resolve and copy the underlying file
            let resolved = std::fs::canonicalize(&target_path)
                .with_context(|| format!("canonicalize {}", target_path.display()))?;
            std::fs::copy(&resolved, &dest)?;
        } else if meta.is_dir() {
            util::copy_dir_recursive(&target_path, &dest)?;
        } else {
            std::fs::copy(&target_path, &dest)?;
        }
        manifest_files.push(manifest::FileMapping {
            source: live_path.clone(),
            target: live_path.clone(),
        });
    }

    let manifest = manifest::Manifest {
        name: name.to_string(),
        description: format!("Captured by `dot-switch new` on {}", chrono::Utc::now().to_rfc3339()),
        files: manifest_files,
    };
    let raw = toml::to_string_pretty(&manifest)?;
    std::fs::write(profile_dir.join("dot-switch.toml"), raw)?;
    println!("Created profile `{}` at {}", name, profile_dir.display());
    println!("Run `dot-switch use {}` to apply it.", name);
    Ok(())
}

fn cmd_config(action: Option<ConfigAction>) -> Result<()> {
    let action = action.unwrap_or(ConfigAction::Show);
    match action {
        ConfigAction::Path => {
            println!("{}", Config::config_path()?.display());
        }
        ConfigAction::Show => {
            let cfg = Config::load_or_init()?;
            let raw = std::fs::read_to_string(Config::config_path()?)?;
            println!("{}", raw);
            let _ = cfg;
        }
        ConfigAction::Edit => {
            let path = Config::config_path()?;
            // Ensure it exists first
            Config::load_or_init()?;
            let editor = dot_switch::cli::resolve_editor();
            let status = StdCommand::new(&editor)
                .arg(&path)
                .status()
                .with_context(|| format!("spawn editor `{}`", editor))?;
            if !status.success() {
                bail!("editor exited with {:?}", status.code());
            }
        }
    }
    Ok(())
}

fn cmd_paths() -> Result<()> {
    let cfg = Config::load_or_init()?;
    println!("config:   {}", Config::config_path()?.display());
    println!("state:    {}", Config::state_path()?.display());
    println!("backups:  {}", Config::backups_dir()?.display());
    println!("profiles: {}", cfg.profiles_repo);
    println!("xdg_cfg:  {}", switch::xdg_config_home()?.display());
    Ok(())
}

fn cmd_completions(shell: clap_complete::Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = "dot-switch".to_string();
    generate(shell, &mut cmd, &name, &mut std::io::stdout());
    Ok(())
}
