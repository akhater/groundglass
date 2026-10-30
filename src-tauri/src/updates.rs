//! Is there a newer Groundglass? Reads GitHub's answer about the latest
//! **published release** of akhater/groundglass. A commit on `main` or a branch
//! never counts, nor do drafts and pre-releases: only what a release build was
//! made for.
//!
//! The screen does the asking (`useNewRelease.ts`): the web view has HTTPS and
//! the phone's own network, where `ureq` here is plain HTTP for the camera.
//! Nothing is downloaded or installed: the app shows the new version and opens
//! the release page.

use serde::Deserialize;

/// The version running.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Where a new version is downloaded from.
pub const RELEASES_PAGE: &str = "https://github.com/akhater/groundglass/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// The version in GitHub's `answer` if it is later than `current`. An answer
/// that isn't a release (a Wi-Fi sign-in page) is an error, not "nothing new".
pub fn newer_version(answer: &str, current: &str) -> Result<Option<String>, serde_json::Error> {
    let release: Release = serde_json::from_str(answer)?;
    let version = clean(&release.tag_name);
    let published = !release.draft && !release.prerelease;
    Ok((published && is_newer(&version, current)).then_some(version))
}

/// "v26.43.1", "Groundglass v26.43.1" -> "26.43.1".
fn clean(tag: &str) -> String {
    tag.trim().trim_start_matches(|c: char| !c.is_ascii_digit()).to_string()
}

/// "26.43.1" -> [26, 43, 1]. A suffix ("-beta", "+build") is set aside.
fn numbers(version: &str) -> Option<Vec<u64>> {
    let core = clean(version);
    let core = core.split(['-', '+']).next()?;
    core.split('.').map(|n| n.parse().ok()).collect()
}

/// `latest` is a later version than `current` ("26.43.1" > "26.42.9";
/// "26.43" = "26.43.0"). Anything that isn't a version is never newer.
fn is_newer(latest: &str, current: &str) -> bool {
    let (Some(mut l), Some(mut c)) = (numbers(latest), numbers(current)) else {
        return false;
    };
    let n = l.len().max(c.len());
    l.resize(n, 0);
    c.resize(n, 0);
    l > c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, prerelease: bool) -> String {
        serde_json::json!({ "tag_name": tag, "draft": draft, "prerelease": prerelease, "body": "notes" }).to_string()
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("26.43.1", "26.42.1"));
        assert!(is_newer("v26.42.2", "26.42.1"));
        assert!(is_newer("26.42.10", "26.42.9"));
        assert!(is_newer("27.1.0", "26.52.3"));
        assert!(!is_newer("26.42.1", "26.42.1"));
        assert!(!is_newer("v26.42.1", "26.42.1"));
        assert!(!is_newer("26.41.5", "26.42.1"));
        assert!(!is_newer("26.42", "26.42.0"));
        assert!(is_newer("26.42.0.1", "26.42"));
    }

    #[test]
    fn odd_tags_never_announce_anything() {
        assert!(!is_newer("", "26.42.1"));
        assert!(!is_newer("latest", "26.42.1"));
        assert!(!is_newer("v26.x", "26.42.1"));
        assert!(is_newer("Groundglass v26.43.1", "26.42.1"));
        assert!(is_newer("26.43.0-beta", "26.42.1"));
    }

    #[test]
    fn tags_lose_their_prefix() {
        assert_eq!(clean("v26.43.1"), "26.43.1");
        assert_eq!(clean("Groundglass v26.43.1"), "26.43.1");
        assert_eq!(clean("26.43.1"), "26.43.1");
    }

    #[test]
    fn a_later_release_is_announced_by_its_version() {
        let answer = release("v26.45.1", false, false);
        assert_eq!(newer_version(&answer, "26.44.5").unwrap().as_deref(), Some("26.45.1"));
    }

    #[test]
    fn the_running_version_or_an_older_one_announces_nothing() {
        assert_eq!(newer_version(&release("26.44.5", false, false), "26.44.5").unwrap(), None);
        assert_eq!(newer_version(&release("26.44.4", false, false), "26.44.5").unwrap(), None);
    }

    #[test]
    fn a_tag_that_is_not_a_version_announces_nothing() {
        assert_eq!(newer_version(&release("nightly", false, false), "26.44.5").unwrap(), None);
    }

    #[test]
    fn drafts_and_pre_releases_announce_nothing() {
        assert_eq!(newer_version(&release("26.45.1", true, false), "26.44.5").unwrap(), None);
        assert_eq!(newer_version(&release("26.45.1", false, true), "26.44.5").unwrap(), None);
    }

    #[test]
    fn a_page_that_is_not_a_release_is_an_error() {
        assert!(newer_version("<html>Sign in to this Wi-Fi</html>", "26.44.5").is_err());
        assert!(newer_version("{\"message\":\"rate limited\"}", "26.44.5").is_err());
    }

    #[test]
    fn the_running_version_is_a_version() {
        assert!(is_newer("99999", CURRENT));
    }
}
