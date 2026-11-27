//! Plan `rename-oratype`: Typelite became OraType in 1.2.0, bundle identifier included
//! (`dev.typelite.mac` -> `dev.oratype.mac`), and OraType later became Munia. macOS keys an app's
//! folders and login item by its identifier and name, so on the first launch after an update
//! this moves legacy data across and cleans up stale login items before anything reads them:
//!
//! - `~/Library/Application Support/<old id>` -> `<new id>`: settings, the database (renamed
//!   `oratype.db`) and the downloaded models. A move on the same disk, so gigabytes of models
//!   take no time.
//! - `~/Library/WebKit/<old id>` -> `<new id>`: the windows' local storage.
//! - The installed bundle path stays valid during an in-place update. Munia's name and
//!   icon come from the new bundle metadata. New downloads use Munia.app.
//! - Legacy LaunchAgents are removed only after Munia reconciles the saved login preference.
//!
//! API keys move on first use (`credentials.rs`). macOS permissions (Accessibility, Microphone,
//! Automation) belong to the identifier and cannot be moved: they are granted again once from
//! Typelite. Everything here is a no-op when there is nothing to move, so it runs on every launch.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::Path;

#[cfg(not(feature = "dev-build"))]
mod names {
    pub const OLD_ID: &str = "dev.typelite.mac";
    pub const NEW_ID: &str = "dev.oratype.mac";
    pub const LEGACY_NAMES: [&str; 2] = ["Typelite", "OraType"];
}
#[cfg(feature = "dev-build")]
mod names {
    pub const OLD_ID: &str = "dev.typelite.mac.dev";
    pub const NEW_ID: &str = "dev.oratype.mac.dev";
    pub const LEGACY_NAMES: [&str; 2] = ["Typelite Dev", "OraType Dev"];
}
use names::{LEGACY_NAMES, NEW_ID, OLD_ID};

/// The database Typelite kept in its data folder, and its SQLite side files.
const OLD_DATABASE: &str = "typelite.db";
const NEW_DATABASE: &str = "oratype.db";
const DATABASE_SIDE_FILES: [&str; 3] = ["", "-wal", "-shm"];

/// Moves legacy data before the app reads its settings (macOS only).
pub fn run() {
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        move_folders(Path::new(&home));
    }
}

/// Moves Typelite's folders to `NEW_ID` under `home`.
fn move_folders(home: &Path) {
    let library = home.join("Library");
    for parent in ["Application Support", "WebKit"] {
        let parent = library.join(parent);
        move_folder(&parent.join(OLD_ID), &parent.join(NEW_ID));
    }
    rename_database(&library.join("Application Support").join(NEW_ID));
}

fn legacy_login_items(home: &Path) -> impl Iterator<Item = std::path::PathBuf> + '_ {
    LEGACY_NAMES.iter().map(move |name| {
        home.join("Library/LaunchAgents")
            .join(format!("{name}.plist"))
    })
}

/// Keeps a former login item available when registering its replacement fails.
pub(crate) fn has_legacy_login_item(home: &Path) -> bool {
    legacy_login_items(home).any(|agent| agent.is_file())
}

/// Retires the old items after the current login preference has been applied.
pub(crate) fn remove_legacy_login_items(home: &Path) {
    for agent in legacy_login_items(home) {
        match std::fs::remove_file(&agent) {
            Ok(()) => tracing::info!("Rename: removed login item {}", agent.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(
                "Rename: could not remove login item {}: {error}",
                agent.display()
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            assert!(agent.exists());
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
        let munia_agent = agents_dir.join(format!("{}.plist", crate::APP_NAME));
        write(&munia_agent, "munia");

        assert!(has_legacy_login_item(&home));
        remove_legacy_login_items(&home);
        assert!(!has_legacy_login_item(&home));

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
}
