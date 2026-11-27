//! Plan `rename-oratype`: Typelite became OraType in 1.2.0, bundle identifier included
//! (`dev.typelite.mac` -> `dev.oratype.mac`), and OraType later became Munia. macOS keys an app's
//! folders and login item by its identifier and name, so on the first launch after an update
//! this moves legacy data across and cleans up stale login items before anything reads them:
//!
//! - `~/Library/Application Support/<old id>` -> `<new id>`: settings, the database (renamed
//!   `oratype.db`) and the downloaded models. A move on the same disk, so gigabytes of models
//!   take no time.
//! - `~/Library/WebKit/<old id>` -> `<new id>`: the windows' local storage.
//! - `~/Library/LaunchAgents/<legacy name>.plist` (`Typelite` and `OraType`) is removed.
//!   "Launch at login" registers Munia again from the setting (`sync_auto_start_preference`).
//! - An app bundle still called `Typelite.app` or `OraType.app` is renamed `Munia.app` and started
//!   again from there: an in-place update keeps the old folder name.
//!
//! API keys move on first use (`credentials.rs`). macOS permissions (Accessibility, Microphone,
//! Automation) belong to the identifier and cannot be moved: they are granted again once from
//! Typelite. Everything here is a no-op when there is nothing to move, so it runs on every launch.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::{Path, PathBuf};

#[cfg(not(feature = "dev-build"))]
mod names {
    pub const OLD_ID: &str = "dev.typelite.mac";
    pub const NEW_ID: &str = "dev.oratype.mac";
    pub const LEGACY_NAMES: [&str; 2] = ["Typelite", "OraType"];
    pub const NEW_NAME: &str = "Munia";
}
#[cfg(feature = "dev-build")]
mod names {
    pub const OLD_ID: &str = "dev.typelite.mac.dev";
    pub const NEW_ID: &str = "dev.oratype.mac.dev";
    pub const LEGACY_NAMES: [&str; 2] = ["Typelite Dev", "OraType Dev"];
    pub const NEW_NAME: &str = "Munia Dev";
}
use names::{LEGACY_NAMES, NEW_ID, NEW_NAME, OLD_ID};

/// The database Typelite kept in its data folder, and its SQLite side files.
const OLD_DATABASE: &str = "typelite.db";
const NEW_DATABASE: &str = "oratype.db";
const DATABASE_SIDE_FILES: [&str; 3] = ["", "-wal", "-shm"];

/// What the caller does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Carry on starting.
    Continue,
    /// The bundle was renamed and the app started again from its new place: exit now.
    Exit,
}

/// Runs the migration (macOS only). Call first thing at startup, after logging is set up.
pub fn run() -> Next {
    #[cfg(target_os = "macos")]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Next::Continue;
        };
        move_folders(&home);
        if let Ok(exe) = std::env::current_exe() {
            if let Some((from, to)) = bundle_rename(&exe) {
                return relaunch_renamed(&from, &to);
            }
        }
    }
    Next::Continue
}

/// Moves Typelite's folders to `NEW_ID` under `home`, and removes legacy login items.
fn move_folders(home: &Path) {
    let library = home.join("Library");
    for parent in ["Application Support", "WebKit"] {
        let parent = library.join(parent);
        move_folder(&parent.join(OLD_ID), &parent.join(NEW_ID));
    }
    rename_database(&library.join("Application Support").join(NEW_ID));
    remove_legacy_login_items(&library);
}

/// Removes stale LaunchAgents plists left by earlier product names (`Typelite` and `OraType`)
/// so `sync_auto_start_preference` re-registers `Munia.plist` pointing at the current app.
fn remove_legacy_login_items(library: &Path) {
    let agents_dir = library.join("LaunchAgents");
    for legacy_name in LEGACY_NAMES {
        let agent = agents_dir.join(format!("{legacy_name}.plist"));
        if agent.exists() {
            match std::fs::remove_file(&agent) {
                Ok(()) => tracing::info!("Rename: removed {legacy_name}'s login item"),
                Err(error) => {
                    tracing::warn!("Rename: could not remove {legacy_name}'s login item: {error}")
                }
            }
        }
    }
}

/// Moves `from` to `to` when only `from` exists. When both exist the current data folder has
/// run before, so it wins and Typelite's is left alone.
fn move_folder(from: &Path, to: &Path) {
    if !from.is_dir() {
        return;
    }
    if to.exists() {
        tracing::info!(
            "Rename: {} and {} both exist; keeping existing folder",
            from.display(),
            to.display()
        );
        return;
    }
    match std::fs::rename(from, to) {
        Ok(()) => tracing::info!("Rename: moved {} to {}", from.display(), to.display()),
        Err(error) => tracing::warn!("Rename: could not move {}: {error}", from.display()),
    }
}

/// Renames `typelite.db` (and its `-wal` / `-shm` files) to `oratype.db` in `dir`, unless
/// `oratype.db` already exists.
fn rename_database(dir: &Path) {
    if !dir.join(OLD_DATABASE).exists() || dir.join(NEW_DATABASE).exists() {
        return;
    }
    for suffix in DATABASE_SIDE_FILES {
        let from = dir.join(format!("{OLD_DATABASE}{suffix}"));
        if from.exists() {
            if let Err(error) = std::fs::rename(&from, dir.join(format!("{NEW_DATABASE}{suffix}")))
            {
                tracing::warn!("Rename: could not rename {}: {error}", from.display());
            }
        }
    }
    tracing::info!("Rename: renamed the database to {NEW_DATABASE}");
}

/// The bundle to rename and its new path, when the running app sits in a bundle still called
/// `Typelite.app` or `OraType.app` (an update replaces the bundle's contents in place) and nothing
/// is in the way. A bundle macOS runs from a randomised read-only copy (App Translocation, for a
/// downloaded app opened in place) is left alone.
fn bundle_rename(exe: &Path) -> Option<(PathBuf, PathBuf)> {
    let bundle = exe
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))?;
    let file_name = bundle.file_name()?;
    let is_legacy_bundle = LEGACY_NAMES
        .iter()
        .any(|legacy_name| file_name == std::ffi::OsStr::new(&format!("{legacy_name}.app")));
    if !is_legacy_bundle {
        return None;
    }
    if bundle
        .components()
        .any(|part| part.as_os_str() == "AppTranslocation")
    {
        return None;
    }
    let target = bundle.parent()?.join(format!("{NEW_NAME}.app"));
    (!target.exists()).then(|| (bundle.to_path_buf(), target))
}

/// Renames the bundle and opens it from its new path. On failure the app just keeps running
/// from the old one.
#[cfg(target_os = "macos")]
fn relaunch_renamed(from: &Path, to: &Path) -> Next {
    if let Err(error) = std::fs::rename(from, to) {
        tracing::warn!("Rename: could not rename {}: {error}", from.display());
        return Next::Continue;
    }
    tracing::info!("Rename: renamed {} to {}", from.display(), to.display());
    // `open -n` starts a new instance from the new path; this one exits before it sets up
    // anything (single instance, tray, shortcuts).
    match std::process::Command::new("/usr/bin/open")
        .arg("-n")
        .arg(to)
        .spawn()
    {
        Ok(_) => Next::Exit,
        Err(error) => {
            tracing::warn!("Rename: could not start {}: {error}", to.display());
            Next::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("munia-rename-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn typelites_folders_move_across_with_the_database_renamed() {
        let home = temp_home("move");
        let old = home.join("Library/Application Support").join(OLD_ID);
        write(&old.join("settings.json"), "{}");
        write(&old.join("models/ggml-base-q5_1.bin"), "model");
        write(&old.join(OLD_DATABASE), "db");
        write(&old.join(format!("{OLD_DATABASE}-wal")), "wal");
        write(&home.join("Library/WebKit").join(OLD_ID).join("data"), "x");
        let legacy_agents: Vec<PathBuf> = LEGACY_NAMES
            .iter()
            .map(|name| {
                let path = home
                    .join("Library/LaunchAgents")
                    .join(format!("{name}.plist"));
                write(&path, "plist");
                path
            })
            .collect();

        move_folders(&home);

        let new = home.join("Library/Application Support").join(NEW_ID);
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(new.join("models/ggml-base-q5_1.bin").exists());
        assert_eq!(
            std::fs::read_to_string(new.join(NEW_DATABASE)).unwrap(),
            "db"
        );
        assert!(new.join(format!("{NEW_DATABASE}-wal")).exists());
        assert!(!new.join(OLD_DATABASE).exists());
        assert!(home
            .join("Library/WebKit")
            .join(NEW_ID)
            .join("data")
            .exists());
        for agent in &legacy_agents {
            assert!(!agent.exists());
        }
        // A second run has nothing to do.
        move_folders(&home);
        assert!(new.join(NEW_DATABASE).exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn stale_typelite_and_oratype_login_items_are_removed_while_munia_stays() {
        let home = temp_home("login");
        let agents_dir = home.join("Library/LaunchAgents");
        let legacy_agents: Vec<PathBuf> = LEGACY_NAMES
            .iter()
            .map(|name| {
                let path = agents_dir.join(format!("{name}.plist"));
                write(&path, "legacy");
                path
            })
            .collect();
        let munia_agent = agents_dir.join(format!("{NEW_NAME}.plist"));
        write(&munia_agent, "munia");

        move_folders(&home);

        for agent in &legacy_agents {
            assert!(!agent.exists());
        }
        assert!(munia_agent.exists());
        assert_eq!(std::fs::read_to_string(&munia_agent).unwrap(), "munia");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn an_existing_oratype_folder_wins() {
        let home = temp_home("both");
        let support = home.join("Library/Application Support");
        write(&support.join(OLD_ID).join("settings.json"), "old");
        write(&support.join(NEW_ID).join("settings.json"), "new");
        move_folders(&home);
        assert_eq!(
            std::fs::read_to_string(support.join(NEW_ID).join("settings.json")).unwrap(),
            "new"
        );
        assert!(support.join(OLD_ID).exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn legacy_typelite_and_oratype_bundles_are_renamed_to_munia() {
        let apps = temp_home("bundle");
        let exe = |bundle: &str| apps.join(bundle).join("Contents/MacOS/oratype");
        for legacy_name in LEGACY_NAMES {
            assert_eq!(
                bundle_rename(&exe(&format!("{legacy_name}.app"))),
                Some((
                    apps.join(format!("{legacy_name}.app")),
                    apps.join(format!("{NEW_NAME}.app"))
                ))
            );
        }
        // Already renamed, renamed by the user, or not in a bundle (a development run).
        assert_eq!(bundle_rename(&exe(&format!("{NEW_NAME}.app"))), None);
        assert_eq!(bundle_rename(&exe("My Dictation.app")), None);
        assert_eq!(bundle_rename(Path::new("/repo/target/debug/oratype")), None);
        // Something already called Munia.app is in the way.
        std::fs::create_dir_all(apps.join(format!("{NEW_NAME}.app"))).unwrap();
        for legacy_name in LEGACY_NAMES {
            assert_eq!(bundle_rename(&exe(&format!("{legacy_name}.app"))), None);
            // A translocated copy is read-only.
            let translocated = PathBuf::from(format!(
                "/private/var/folders/x/AppTranslocation/ABC/d/{legacy_name}.app/Contents/MacOS/oratype"
            ));
            assert_eq!(bundle_rename(&translocated), None);
        }
        let _ = std::fs::remove_dir_all(&apps);
    }
}
