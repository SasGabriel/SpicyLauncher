use crate::progress::ProgressBar;
use anyhow::{anyhow, Result};
use colored::Colorize;
use spicy_launcher_core::github::GitHubClient;
use spicy_launcher_core::release::Release;
use spicy_launcher_core::storage::LocalStorage;
use spicy_launcher_core::tracker::{ProgressEvent, ProgressTracker};
use spicy_launcher_core::Game;

pub struct App {
    client: GitHubClient,
    storage: LocalStorage,
    progress_bar: ProgressBar,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: GitHubClient::new()?,
            storage: LocalStorage::init()?,
            progress_bar: ProgressBar::default(),
        })
    }

    async fn get_releases(&self, game: Game) -> Result<Vec<Release>> {
        self.progress_bar.enable_tick();
        self.progress_bar.set_message("Updating... Please wait.");
        Ok(self.client.get_releases(game).await?)
    }

    pub fn find_version(&self, version: Option<String>, releases: Vec<Release>) -> Result<Release> {
        if releases.is_empty() {
            return Err(anyhow!("No releases found/installed :("));
        }

        match version {
            Some(version) => {
                let normalized_version = if version.starts_with('v') {
                    version.clone()
                } else {
                    format!("v{}", version)
                };

                releases
                    .clone()
                    .into_iter()
                    .find(|release| release.version == normalized_version)
                    .ok_or_else(|| {
                        anyhow!(
                            "Version {} not found, available versions are: {}",
                            version.red(),
                            releases
                                .iter()
                                .enumerate()
                                .map(|(i, release)| if i != releases.len() - 1 {
                                    format!("{},", release.version)
                                } else {
                                    release.version.to_string()
                                })
                                .collect::<Vec<String>>()
                                .join(" ")
                                .blue()
                        )
                    })
            }
            None => Ok(releases[0].clone()),
        }
    }

    pub async fn print_releases(&self, game: Game) -> Result<()> {
        let available_relases = self.storage.get_available_releases(game)?;

        let mut releases: Vec<Release> = self.get_releases(game).await?;
        releases.iter_mut().for_each(|release| {
            release.installed = available_relases
                .iter()
                .any(|r| r.version == release.version)
        });

        self.progress_bar.finish();

        println!();
        println!("🐟 {game} - Available versions:");
        for release in releases {
            let release_title = format!(
                "- {} {} ({}) [{}]",
                game,
                release.version.blue(),
                release.name.green(),
                if release.installed {
                    "installed".green().to_string()
                } else {
                    String::from("not installed")
                }
            );

            if release.prerelease {
                println!("{release_title} [{}]", "pre-release".yellow());
            } else {
                println!("{release_title}");
            }
        }
        println!();

        Ok(())
    }

    pub async fn install(&mut self, game: Game, version: Option<String>) -> Result<()> {
        let releases = self.get_releases(game).await?;
        let release = self.find_version(version, releases)?;
        let asset = release.get_asset()?;
        let download_path = self.storage.temp_dir.join(&asset.name);
        self.progress_bar
            .set_total_progress(asset.size, ProgressEvent::Download);
        self.progress_bar
            .set_message(format!("{} {}", "Downloading".blue(), &asset.name,));
        self.client
            .download_asset(&asset, &download_path, &mut self.progress_bar)
            .await?;
        self.progress_bar.reset_style();
        self.progress_bar.enable_tick();
        self.progress_bar
            .set_message(format!("{} {}", "Verifying".yellow(), &asset.name));
        self.client.verify_asset(&asset, &download_path).await?;
        self.progress_bar
            .set_message(format!("{} {}", "Extracting".green(), &asset.name));
        self.storage.extract_archive(
            &asset,
            &download_path,
            game,
            &release.version,
            &mut self.progress_bar,
        )?;
        self.progress_bar.finish();
        log::info!("{} is ready to play! 🐟", &release.version);
        Ok(())
    }

    pub async fn uninstall(&mut self, game: Game, version: Option<String>) -> Result<()> {
        let releases = self.get_releases(game).await?;
        let release = self.find_version(version, releases)?;
        let install_path = self.storage.version_path(game, &release.version);
        if install_path.exists() {
            log::debug!("Removing {:?}", install_path);
            self.progress_bar.set_message(format!(
                "{} {}",
                "Uninstalling".green(),
                &release.version
            ));
            self.storage.remove_version(game, &release.version)?;
            self.progress_bar.finish();
            log::info!("{} is uninstalled.", &release.version);
        } else {
            self.progress_bar.finish();
            log::warn!("{} is not installed.", release.version);
        }
        Ok(())
    }

    pub fn launch(&self, game: Game, version: Option<String>) -> Result<()> {
        let available_relases = self.storage.get_available_releases(game)?;
        let release = self.find_version(version, available_relases)?;
        self.storage.launch_game(game, &release.version)?;
        Ok(())
    }
}

// #[cfg(test)]
// mod tests;

// #[cfg(test)]
// mod tests {
//     // https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html
//     use super::*;

//     // FAKE object
//     // instead of using github api to get releases, we create a fake release object
//     fn create_fake_release(version: &str, name: &str, prerelease: bool) -> Release {
//         Release {
//             version: version.to_string(),  // &str -> String conversion (for ownership)
//             name: name.to_string(),
//             prerelease,
//             installed: false,
//             ..Default::default()  // Use default values for other fields
//         }
//     }

//     // Test 1: find_version with empty releases (error case)
//     #[test]
//     fn test_find_version_with_empty_releases() {
//         let app = App::new().unwrap();
//         let releases = vec![];

//         let result = app.find_version(Some("1.0.0".to_string()), releases);
//         assert!(result.is_err());
//         assert!(result.unwrap_err().to_string().contains("No releases found"));
//     }

//     // Test 2: find_version with existing versions with prefix
//     #[test]
//     fn test_find_version_with_existing_version() {
//         let app = App::new().unwrap();
//         // let releases = vec![Release {
//         //     version: "v1.0.0".to_string(),
//         //     ..Default::default()
//         // }];
//         let releases = vec![create_fake_release("v1.0.0", "Release v1.0.0", false),
//             create_fake_release("v2.0.0", "Release v2.0.0", false)];

//         let result1 = app.find_version(Some("v1.0.0".to_string()), releases.clone());
//         let result2 = app.find_version(Some("v2.0.0".to_string()), releases);
//         assert!(result1.is_ok());
//         assert_eq!(result1.unwrap().version, "v1.0.0");
//         assert!(result2.is_ok());
//         assert_eq!(result2.unwrap().version, "v2.0.0");
//     }

//     // Test 3: find_version with existing versions without prefix and test normalization
//     #[test]
//     fn test_version_normalization() {
//         let app = App::new().unwrap();
//         let releases = vec![
//             create_fake_release("v1.0.0", "Release 1", false),
//             create_fake_release("v1.0.1", "Release 2", false),
//         ];

//         // Test with version starting with 'v'
//         let result1 = app.find_version(Some("v1.0.1".to_string()), releases.clone());
//         assert!(result1.is_ok());
//         assert_eq!(result1.unwrap().version, "v1.0.1");

//         // Test with version not starting with 'v' (should add 'v' prefix)
//         let result2 = app.find_version(Some("1.0.0".to_string()), releases);
//         assert!(result2.is_ok());
//         assert_eq!(result2.unwrap().version, "v1.0.0");
//     }

//     // Test 4: find_version with non-existing version (error case)
//     #[test]
//     fn test_find_version_with_non_existing_version() {
//         let app = App::new().unwrap();  // Panic if creation fails

//         let releases = vec![
//             create_fake_release("v1.0.0", "Release 1.0.0", false),
//             create_fake_release("v2.0.0", "Release 2.0.0", false),
//         ];

//         let result = app.find_version(Some("3.0.0".to_string()), releases.clone());

//         assert!(result.is_err());  // Should be an error
//     }

//     // Test 5: find_version with None (should return first release)
//     #[test]
//     fn test_find_version_with_none_returns_first() {
//         let app = App::new().unwrap();
//         let releases = vec![
//             create_fake_release("v2.0.0", "Latest Release", false),
//             create_fake_release("v1.0.0", "Older Release", false),
//         ];

//         let result = app.find_version(None, releases);
//         assert!(result.is_ok());
//         assert_eq!(result.unwrap().version, "v2.0.0");
//     }

//     // Test 6: Test prerelease version handling (special characters in string)
//     #[test]
//     fn test_find_version_with_prerelease() {
//         let app = App::new().unwrap();
//         let releases = vec![
//             create_fake_release("v1.0.0", "Stable Release", false),
//             create_fake_release("v1.1.0-beta", "Beta Release", true),
//         ];

//         let result = app.find_version(Some("1.1.0-beta".to_string()), releases);
//         assert!(result.is_ok());
//         let found_release = result.unwrap();
//         assert_eq!(found_release.version, "v1.1.0-beta");
//         assert!(found_release.prerelease);
//     }

// }
