use elemental_core::minecraft::MinecraftVersionId;
use elemental_schema::{forge::ForgeInstallerProfile, mojang::piston::PistonMetaData};

use crate::{
    catalog::{InstallerMavenCatalog, LoaderRelease},
    descriptors::NEOFORGE_DRIVER,
    driver::InstalledDriver,
    error::Result,
    families::installer::{
        InstallerFamily, InstallerFamilyDriver, InstallerMavenSource, MavenEndpoints, MavenSpec,
        profile_game_and_raw_loader_version,
    },
    inspect::{LibraryPrefixSet, ProfileIdPattern, ProfiledDriverIdentity},
    loader_version::LoaderVersionId,
    url::Origin,
};

const GAME_VERSION_HEURISTIC_DESCRIPTION: &str = "Game version is inferred from NeoForge version naming. Installer profile remains the source of truth.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NeoForgeOrigin;

impl Origin for NeoForgeOrigin {
    fn canonical(self) -> &'static str {
        "https://maven.neoforged.net/releases"
    }

    fn all() -> &'static [Self] {
        &[Self]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NeoForgeSpec;

impl MavenSpec for NeoForgeSpec {
    type Origin = NeoForgeOrigin;

    const SOURCE_NAME: &'static str = "neoforge source";
    const MAVEN_ORIGIN: NeoForgeOrigin = NeoForgeOrigin;
    const GROUP_PATH: &'static str = "net/neoforged";
    const COORDINATE_GROUP: &'static str = "net.neoforged";
    const ARTIFACT: &'static str = "neoforge";

    fn version(_game_version: &MinecraftVersionId, loader_version: &LoaderVersionId) -> String {
        loader_version.as_str().to_owned()
    }

    fn release(version: String) -> Option<(MinecraftVersionId, LoaderRelease)> {
        let game_version = infer_neoforge_game_version(version.as_str())?;
        let loader_version_is_prerelease = version.ends_with("-beta");
        Some((
            game_version.clone(),
            LoaderRelease::new(game_version, LoaderVersionId::from(version))
                .with_description(Some(GAME_VERSION_HEURISTIC_DESCRIPTION.to_owned()))
                .with_prerelease(Some(loader_version_is_prerelease)),
        ))
    }
}

pub type NeoForgeEndpoints = MavenEndpoints<NeoForgeSpec>;
pub type NeoForgeSource = InstallerMavenSource<NeoForgeEndpoints>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NeoForgeFamily;

impl InstallerFamily for NeoForgeFamily {
    type Source = NeoForgeSource;
    type Endpoints = NeoForgeEndpoints;

    const IDENTITY: ProfiledDriverIdentity = NEOFORGE_IDENTITY;
    const FAMILY_NAME: &'static str = "neoforge";
    const LOADER_VERSION_PREFIX: &'static str = "neoforge-";

    fn profile_identity(
        install_profile: &ForgeInstallerProfile,
    ) -> Result<(String, LoaderVersionId)> {
        let (game_version, raw_version) =
            profile_game_and_raw_loader_version(install_profile, Self::FAMILY_NAME, "neoforge")?;
        // Maven paths already carry a bare release; strip only exact display prefixes.
        let prefix = format!("{game_version}-{}", Self::LOADER_VERSION_PREFIX);
        let loader_version = raw_version
            .as_str()
            .strip_prefix(&prefix)
            .or_else(|| {
                raw_version
                    .as_str()
                    .strip_prefix(Self::LOADER_VERSION_PREFIX)
            })
            .unwrap_or(raw_version.as_str());
        Ok((game_version, LoaderVersionId::from(loader_version)))
    }

    fn inspect_installed(metadata: &PistonMetaData) -> Option<InstalledDriver> {
        let mut installed = Self::IDENTITY.inspect_installed(metadata)?;
        if installed.driver_version.is_none() {
            let game_prefix = format!(
                "{}-neoforge-",
                metadata.inherits_from.as_deref().unwrap_or_default()
            );
            installed.driver_version = metadata
                .id
                .strip_prefix(&game_prefix)
                .filter(|version| !version.is_empty())
                .map(str::to_owned);
        }
        // FML component versions do not identify the NeoForge release.
        Some(installed)
    }
}

pub type NeoForgeDriver = InstallerFamilyDriver<NeoForgeFamily>;

const NEOFORGE_MARKERS: LibraryPrefixSet = LibraryPrefixSet::new(&[
    "net.neoforged:neoforge:",
    "net.neoforged:forge:",
    "net.neoforged:fmlloader:",
    "net.neoforged.fancymodloader:",
]);
const NEOFORGE_VERSION_LIBRARIES: LibraryPrefixSet = LibraryPrefixSet::new(&[
    "net.neoforged:neoforge:",
    "net.neoforged:forge:",
    "net.neoforged:fmlloader:",
]);
const NEOFORGE_IDENTITY: ProfiledDriverIdentity = ProfiledDriverIdentity::new(
    NEOFORGE_DRIVER,
    NEOFORGE_MARKERS,
    NEOFORGE_VERSION_LIBRARIES,
    ProfileIdPattern::loader_prefixed("neoforge-"),
);

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn profile_identity_accepts_bare_and_exact_family_prefixes() {
        for version in ["21.1.265", "neoforge-21.1.265", "1.21.1-neoforge-21.1.265"] {
            let profile: ForgeInstallerProfile = serde_json::from_value(serde_json::json!({
                "minecraft": "1.21.1", "version": version
            }))
            .unwrap();
            let (game_version, loader_version) =
                NeoForgeFamily::profile_identity(&profile).unwrap();
            assert_eq!(game_version, "1.21.1");
            assert_eq!(loader_version.as_str(), "21.1.265");
        }
    }

    #[test]
    fn profile_identity_preserves_wrong_game_prefixes_and_prefers_maven_path() {
        let mut profile: ForgeInstallerProfile = serde_json::from_value(serde_json::json!({
            "minecraft": "1.21.1", "version": "1.20.1-neoforge-21.1.265"
        }))
        .unwrap();
        assert_eq!(
            NeoForgeFamily::profile_identity(&profile)
                .unwrap()
                .1
                .as_str(),
            "1.20.1-neoforge-21.1.265"
        );
        profile.path = Some("net.neoforged:neoforge:21.1.265".to_owned());
        assert_eq!(
            NeoForgeFamily::profile_identity(&profile)
                .unwrap()
                .1
                .as_str(),
            "21.1.265"
        );
    }

    fn metadata(id: &str, libraries: &[&str]) -> PistonMetaData {
        serde_json::from_value(serde_json::json!({
            "id": id, "inheritsFrom": "1.21.1", "type": "release",
            "assetIndex": {"id": "17", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
            "assets": "17", "complianceLevel": 1,
            "downloads": {"client": {"sha1": "", "size": 0, "url": ""}},
            "javaVersion": {"component": "java-runtime", "majorVersion": 21},
            "libraries": libraries.iter().map(|name| serde_json::json!({"name": name, "downloads": {}})).collect::<Vec<_>>(),
            "mainClass": "example.Main", "minimumLauncherVersion": 21,
            "time": "2024-01-01T00:00:00Z", "releaseTime": "2024-01-01T00:00:00Z"
        })).unwrap()
    }

    #[test]
    fn inspect_keeps_modern_fml_component_versions_unknown() {
        let metadata = metadata(
            "custom-instance",
            &["net.neoforged.fancymodloader:loader:4.0.45"],
        );
        let installed = NeoForgeFamily::inspect_installed(&metadata).unwrap();
        assert_eq!(installed.driver.id, "neoforge");
        assert_eq!(installed.driver_version, None);
    }

    #[test]
    fn inspect_accepts_canonical_profiles_without_marker_libraries() {
        let metadata = metadata("neoforge-21.1.265", &[]);
        assert_eq!(
            NeoForgeFamily::inspect_installed(&metadata)
                .unwrap()
                .driver_version
                .as_deref(),
            Some("21.1.265")
        );
    }

    #[test]
    fn inspect_legacy_profile_id_requires_the_inherited_game_prefix() {
        for (id, expected) in [
            ("1.21.1-neoforge-21.1.265", Some("21.1.265")),
            ("1.20.1-neoforge-21.1.265", None),
        ] {
            let metadata = metadata(id, &["net.neoforged.fancymodloader:loader:4.0.45"]);
            assert_eq!(
                NeoForgeFamily::inspect_installed(&metadata)
                    .unwrap()
                    .driver_version
                    .as_deref(),
                expected
            );
        }
    }
}

pub type NeoForgeCatalog = InstallerMavenCatalog<NeoForgeEndpoints>;

fn infer_neoforge_game_version(loader_version: &str) -> Option<MinecraftVersionId> {
    let core_version = loader_version
        .strip_suffix("-beta")
        .unwrap_or(loader_version);
    let parts = core_version.split('.').collect::<Vec<&str>>();
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    let [major, minor, ..] = parts.as_slice() else {
        return None;
    };

    let major_number = major.parse::<u32>().ok()?;
    if major_number >= 26 {
        let [_, _, hotfix, _build] = parts.as_slice() else {
            return None;
        };
        let game_version = if *hotfix == "0" {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{hotfix}")
        };
        return Some(MinecraftVersionId::from(game_version));
    }

    if parts.len() != 3 {
        return None;
    }
    // Mojang uses "1.21", rather than "1.21.0", for the base release.
    let game_version = if *minor == "0" {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    };
    Some(MinecraftVersionId::from(game_version))
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn releases_match_mojang_base_names_and_year_hotfixes() {
        for (version, game, prerelease) in [
            ("21.0.167", "1.21", false),
            ("21.1.265", "1.21.1", false),
            ("26.1.0.1-beta", "26.1", true),
            ("26.1.1.4-beta", "26.1.1", true),
            ("26.1.2.125", "26.1.2", false),
            ("26.3.0.100", "26.3", false),
        ] {
            let (game_version, release) = NeoForgeSpec::release(version.to_owned()).unwrap();
            assert_eq!(game_version.as_str(), game);
            assert_eq!(release.game_version, game_version);
            assert_eq!(release.loader_version.as_str(), version);
            assert_eq!(release.prerelease, Some(prerelease));
        }
    }

    #[test]
    fn release_inference_rejects_incomplete_or_unknown_formats() {
        for version in [
            "21",
            "21.1",
            "21.1.2.3",
            "26.3.0",
            "26.3.0.1.2",
            "26..0.1",
            "26.3.0../bad",
            "26.3.0.0-alpha.1+snapshot-1",
            "26.3.0.1-rc1",
            "26.3.0.1-beta-extra",
            "26.x.0.1",
            "+26.3.0.1",
            "26.3.x.1",
            "26.3.0.x",
        ] {
            assert!(
                NeoForgeSpec::release(version.to_owned()).is_none(),
                "{version}"
            );
        }
    }
}
