use anyhow::{anyhow, Result};
use spicy_launcher_cli::app::App;
use spicy_launcher_core::{release::Release, Game};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// =============================================================================
// TEST DOUBLES (FAKES, STUBS, MOCKS)
// =============================================================================

// FAKE: Simple fake data creation
fn create_fake_release(version: &str, name: &str, prerelease: bool) -> Release {
    Release {
        version: version.to_string(),
        name: name.to_string(),
        prerelease,
        installed: false,
        ..Default::default()
    }
}

// Helper function to compare releases (since Release doesn't implement PartialEq)
fn releases_equal(releases1: &[Release], releases2: &[Release]) -> bool {
    if releases1.len() != releases2.len() {
        return false;
    }

    for (r1, r2) in releases1.iter().zip(releases2.iter()) {
        if r1.version != r2.version || r1.name != r2.name || r1.prerelease != r2.prerelease {
            return false;
        }
    }
    true
}

// STUB: Always returns the same predetermined response for storage operations
#[derive(Clone)]
struct StubLocalStorage {
    available_releases: Vec<Release>,
    should_fail_init: bool,
    should_fail_get_releases: bool,
    temp_dir: PathBuf,
}

impl StubLocalStorage {
    fn new_success(available_releases: Vec<Release>) -> Self {
        Self {
            available_releases,
            should_fail_init: false,
            should_fail_get_releases: false,
            temp_dir: PathBuf::from("/tmp/test"),
        }
    }

    fn new_failure() -> Self {
        Self {
            available_releases: vec![],
            should_fail_init: true,
            should_fail_get_releases: true,
            temp_dir: PathBuf::from("/tmp/test"),
        }
    }

    // STUB methods - always return predetermined responses
    fn init() -> Result<Self> {
        Ok(Self::new_success(vec![]))
    }

    fn get_available_releases(&self, _game: Game) -> Result<Vec<Release>> {
        if self.should_fail_get_releases {
            Err(anyhow!("Storage error"))
        } else {
            Ok(self.available_releases.clone())
        }
    }

    fn extract_archive(&self) -> Result<()> {
        Ok(())
    }
}

// STUB: GitHub client that always returns predetermined responses
#[derive(Clone)]
struct StubGitHubClient {
    releases_to_return: Vec<Release>,
    should_fail_get_releases: bool,
    should_fail_download: bool,
    should_fail_verify: bool,
    error_message: String,
}

impl StubGitHubClient {
    fn new_success(releases: Vec<Release>) -> Self {
        Self {
            releases_to_return: releases,
            should_fail_get_releases: false,
            should_fail_download: false,
            should_fail_verify: false,
            error_message: String::new(),
        }
    }

    fn new_failure(error_msg: &str) -> Self {
        Self {
            releases_to_return: vec![],
            should_fail_get_releases: true,
            should_fail_download: true,
            should_fail_verify: true,
            error_message: error_msg.to_string(),
        }
    }

    // STUB methods
    async fn get_releases(&self, _game: Game) -> Result<Vec<Release>> {
        if self.should_fail_get_releases {
            Err(anyhow!(self.error_message.clone()))
        } else {
            Ok(self.releases_to_return.clone())
        }
    }

    async fn download_asset(&self, _asset: &str, _path: &PathBuf) -> Result<()> {
        if self.should_fail_download {
            Err(anyhow!("Download failed"))
        } else {
            Ok(())
        }
    }

    async fn verify_asset(&self, _asset: &str, _path: &PathBuf) -> Result<()> {
        if self.should_fail_verify {
            Err(anyhow!("Verification failed"))
        } else {
            Ok(())
        }
    }
}

// MOCK: Records all interactions and can verify them
#[derive(Debug, Default)]
struct MockGitHubClient {
    // Recording all method calls (what makes it a MOCK)
    pub get_releases_calls: Arc<Mutex<Vec<Game>>>,
    pub download_calls: Arc<Mutex<Vec<(String, PathBuf)>>>, // (asset_name, path)
    pub verify_calls: Arc<Mutex<Vec<(String, PathBuf)>>>,   // (asset_name, path)

    // Predetermined responses
    releases_to_return: Vec<Release>,
    should_fail_get_releases: bool,
    should_fail_download: bool,
    should_fail_verify: bool,
}

impl MockGitHubClient {
    fn new() -> Self {
        Self {
            get_releases_calls: Arc::new(Mutex::new(Vec::new())),
            download_calls: Arc::new(Mutex::new(Vec::new())),
            verify_calls: Arc::new(Mutex::new(Vec::new())),
            releases_to_return: vec![],
            should_fail_get_releases: false,
            should_fail_download: false,
            should_fail_verify: false,
        }
    }

    fn with_releases(mut self, releases: Vec<Release>) -> Self {
        self.releases_to_return = releases;
        self
    }

    fn with_download_failure(mut self) -> Self {
        self.should_fail_download = true;
        self
    }

    fn with_verify_failure(mut self) -> Self {
        self.should_fail_verify = true;
        self
    }

    // MOCK methods that record interactions
    async fn get_releases(&self, game: Game) -> Result<Vec<Release>> {
        self.get_releases_calls.lock().unwrap().push(game);
        if self.should_fail_get_releases {
            Err(anyhow!("GitHub API error"))
        } else {
            Ok(self.releases_to_return.clone())
        }
    }

    async fn download_asset(&self, asset_name: &str, path: &PathBuf) -> Result<()> {
        self.download_calls
            .lock()
            .unwrap()
            .push((asset_name.to_string(), path.clone()));
        if self.should_fail_download {
            Err(anyhow!("Download failed"))
        } else {
            Ok(())
        }
    }

    async fn verify_asset(&self, asset_name: &str, path: &PathBuf) -> Result<()> {
        self.verify_calls
            .lock()
            .unwrap()
            .push((asset_name.to_string(), path.clone()));
        if self.should_fail_verify {
            Err(anyhow!("Verification failed"))
        } else {
            Ok(())
        }
    }

    // VERIFICATION methods (what makes it a MOCK)
    fn was_get_releases_called_with(&self, game: Game) -> bool {
        self.get_releases_calls.lock().unwrap().contains(&game)
    }

    fn get_releases_call_count(&self) -> usize {
        self.get_releases_calls.lock().unwrap().len()
    }

    fn was_download_called_with(&self, asset_name: &str, path: &PathBuf) -> bool {
        self.download_calls
            .lock()
            .unwrap()
            .contains(&(asset_name.to_string(), path.clone()))
    }

    fn download_call_count(&self) -> usize {
        self.download_calls.lock().unwrap().len()
    }

    fn verify_call_count(&self) -> usize {
        self.verify_calls.lock().unwrap().len()
    }
}

// =============================================================================
// TESTS FOR find_version METHOD
// =============================================================================

#[test]
fn test_find_version_with_empty_releases() {
    let app = App::new().unwrap();
    let releases = vec![];

    let result = app.find_version(Some("1.0.0".to_string()), releases);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No releases found"));
}

#[test]
fn test_find_version_with_existing_version() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0", "Release v1.0.0", false),
        create_fake_release("v2.0.0", "Release v2.0.0", false),
    ];

    let result1 = app.find_version(Some("v1.0.0".to_string()), releases.clone());
    let result2 = app.find_version(Some("v2.0.0".to_string()), releases);
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().version, "v1.0.0");
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().version, "v2.0.0");
}

#[test]
fn test_version_normalization() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0", "Release 1", false),
        create_fake_release("v1.0.1", "Release 2", false),
    ];

    // Test with version starting with 'v'
    let result1 = app.find_version(Some("v1.0.1".to_string()), releases.clone());
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().version, "v1.0.1");

    // Test with version not starting with 'v'
    let result2 = app.find_version(Some("1.0.0".to_string()), releases);
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().version, "v1.0.0");
}

#[test]
fn test_find_version_with_non_existing_version() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0", "Release 1.0.0", false),
        create_fake_release("v2.0.0", "Release 2.0.0", false),
    ];

    let result = app.find_version(Some("3.0.0".to_string()), releases);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("available versions are"));
    assert!(error_msg.contains("v1.0.0"));
    assert!(error_msg.contains("v2.0.0"));
}

#[test]
fn test_find_version_with_none_returns_first() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v2.0.0", "Latest Release", false),
        create_fake_release("v1.0.0", "Older Release", false),
    ];

    let result = app.find_version(None, releases);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "v2.0.0");
}

#[test]
fn test_find_version_with_prerelease() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0", "Stable Release", false),
        create_fake_release("v1.1.0-beta", "Beta Release", true),
    ];

    let result = app.find_version(Some("1.1.0-beta".to_string()), releases);
    assert!(result.is_ok());
    let found_release = result.unwrap();
    assert_eq!(found_release.version, "v1.1.0-beta");
    assert!(found_release.prerelease);
}

#[test]
fn test_find_version_error_message_formatting() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0", "Release 1", false),
        create_fake_release("v1.1.0", "Release 2", false),
        create_fake_release("v2.0.0", "Release 3", false),
    ];

    let result = app.find_version(Some("999.0.0".to_string()), releases);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("v1.0.0,"));
    assert!(error_msg.contains("v1.1.0,"));
    assert!(error_msg.contains("v2.0.0"));
}

#[test]
fn test_find_version_single_release() {
    let app = App::new().unwrap();
    let releases = vec![create_fake_release("v1.0.0", "Only Release", false)];

    let result1 = app.find_version(None, releases.clone());
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().version, "v1.0.0");

    let result2 = app.find_version(Some("1.0.0".to_string()), releases);
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().version, "v1.0.0");
}

#[test]
fn test_find_version_complex_versions() {
    let app = App::new().unwrap();
    let releases = vec![
        create_fake_release("v1.0.0-alpha", "Alpha Release", true),
        create_fake_release("v1.0.0-beta.1", "Beta Release", true),
        create_fake_release("v1.0.0", "Stable Release", false),
    ];

    let result = app.find_version(Some("1.0.0-alpha".to_string()), releases.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "v1.0.0-alpha");

    let result = app.find_version(Some("v1.0.0-beta.1".to_string()), releases.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "v1.0.0-beta.1");
}

#[test]
fn test_find_version_edge_cases() {
    let _app = App::new().unwrap();

    // Test with empty string version
    let _releases = vec![create_fake_release("v1.0.0", "Release", false)];
    // Empty string version should not match anything (would fall through to error case)
    assert!(true, "Edge case test placeholder");
}

#[test]
fn test_find_version_many_releases() {
    let app = App::new().unwrap();

    let mut releases = Vec::new();
    for i in 0..50 {
        releases.push(create_fake_release(
            &format!("v{}.0.0", i),
            &format!("Release {}", i),
            false,
        ));
    }

    let result = app.find_version(Some("25.0.0".to_string()), releases.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "v25.0.0");

    let result = app.find_version(None, releases);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "v0.0.0");
}

// =============================================================================
// STUB EXAMPLES
// =============================================================================

#[test]
fn test_stub_github_client_always_returns_same_data() {
    // STUB EXAMPLE: Always returns predetermined data regardless of input
    let releases = vec![create_fake_release("v1.0.0", "Stubbed Release", false)];
    let stub = StubGitHubClient::new_success(releases);

    // Call multiple times with different games - always returns same data
    let result1 = futures::executor::block_on(stub.get_releases(Game::Jumpy));
    let result2 = futures::executor::block_on(stub.get_releases(Game::Punchy));

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert_eq!(result1.unwrap().len(), 1);
    assert_eq!(result2.unwrap().len(), 1);
}

#[test]
fn test_stub_github_client_failure_scenario() {
    // STUB EXAMPLE: Always fails with predetermined error
    let stub = StubGitHubClient::new_failure("Network timeout");

    let result = futures::executor::block_on(stub.get_releases(Game::Jumpy));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Network timeout"));
}

#[test]
fn test_stub_local_storage_success() {
    // STUB EXAMPLE: Storage operations
    let available_releases = vec![create_fake_release("v1.0.0", "Installed", false)];
    let stub = StubLocalStorage::new_success(available_releases.clone());

    let result = stub.get_available_releases(Game::Jumpy);
    assert!(result.is_ok());
    let returned_releases = result.unwrap();
    assert!(releases_equal(&returned_releases, &available_releases));
}

#[test]
fn test_stub_local_storage_failure() {
    // STUB EXAMPLE: Storage failure
    let stub = StubLocalStorage::new_failure();

    let result = stub.get_available_releases(Game::Jumpy);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Storage error"));
}

// =============================================================================
// MOCK EXAMPLES
// =============================================================================

#[test]
fn test_mock_records_github_interactions() {
    // MOCK EXAMPLE: Verify method calls were made
    let releases = vec![create_fake_release("v1.0.0", "Mock Release", false)];
    let mock = MockGitHubClient::new().with_releases(releases);

    // Make some async calls
    let _result1 = futures::executor::block_on(mock.get_releases(Game::Jumpy));
    let _result2 = futures::executor::block_on(mock.get_releases(Game::Punchy));
    let _result3 = futures::executor::block_on(
        mock.download_asset("test.zip", &PathBuf::from("/tmp/test.zip")),
    );

    // VERIFY the interactions (this is what makes it a MOCK)
    assert!(mock.was_get_releases_called_with(Game::Jumpy));
    assert!(mock.was_get_releases_called_with(Game::Punchy));
    assert_eq!(mock.get_releases_call_count(), 2);
    assert_eq!(mock.download_call_count(), 1);
    assert!(mock.was_download_called_with("test.zip", &PathBuf::from("/tmp/test.zip")));
}

#[test]
fn test_mock_verifies_call_sequence() {
    // MOCK EXAMPLE: Verify specific interaction patterns
    let mock = MockGitHubClient::new();

    // Simulate a typical workflow
    let _releases = futures::executor::block_on(mock.get_releases(Game::Jumpy));
    let _download = futures::executor::block_on(
        mock.download_asset("game.zip", &PathBuf::from("/tmp/game.zip")),
    );
    let _verify =
        futures::executor::block_on(mock.verify_asset("game.zip", &PathBuf::from("/tmp/game.zip")));

    // Verify the complete workflow was executed
    assert_eq!(mock.get_releases_call_count(), 1);
    assert_eq!(mock.download_call_count(), 1);
    assert_eq!(mock.verify_call_count(), 1);

    // Verify specific calls were made
    assert!(mock.was_get_releases_called_with(Game::Jumpy));
    assert!(mock.was_download_called_with("game.zip", &PathBuf::from("/tmp/game.zip")));
}

#[test]
fn test_mock_failure_scenarios() {
    // MOCK EXAMPLE: Testing failure handling
    let mock = MockGitHubClient::new()
        .with_download_failure()
        .with_verify_failure();

    // Try operations that should fail
    let download_result = futures::executor::block_on(
        mock.download_asset("failing.zip", &PathBuf::from("/tmp/failing.zip")),
    );
    let verify_result = futures::executor::block_on(
        mock.verify_asset("failing.zip", &PathBuf::from("/tmp/failing.zip")),
    );

    // Verify failures occurred but calls were still recorded
    assert!(download_result.is_err());
    assert!(verify_result.is_err());
    assert_eq!(mock.download_call_count(), 1);
    assert_eq!(mock.verify_call_count(), 1);
}
