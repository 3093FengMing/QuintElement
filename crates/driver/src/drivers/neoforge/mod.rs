use elemental_core::minecraft::{MinecraftVersion, MinecraftVersionId};
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
    const GROUP: &'static str = "net.neoforged";
    const ARTIFACT: &'static str = "neoforge";

    fn version(_game_version: &MinecraftVersionId, loader_version: &LoaderVersionId) -> String {
        loader_version.as_str().to_owned()
    }

    fn release(version: String) -> Option<(MinecraftVersionId, LoaderRelease)> {
        let game_version = infer_neoforge_game_version(version.as_str())?;
        Some((
            game_version.clone(),
            LoaderRelease::new(game_version, LoaderVersionId::from(version))
                .with_description(Some(GAME_VERSION_HEURISTIC_DESCRIPTION.to_owned())),
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

pub type NeoForgeCatalog = InstallerMavenCatalog<NeoForgeEndpoints>;

fn infer_neoforge_game_version(loader_version: &str) -> Option<MinecraftVersionId> {
    let core_version = loader_version.split('-').next().unwrap_or(loader_version);
    let parts = core_version.split('.').collect::<Vec<&str>>();
    let [major, minor, ..] = parts.as_slice() else {
        return None;
    };

    let major_number = major.parse::<u32>().ok()?;
    let minor_number = minor.parse::<u32>().ok()?;
    if major_number >= 26 {
        let patch_number = match parts.get(2) {
            Some(patch) => patch.parse::<u32>().ok()?,
            None => 0,
        };

        return Some(
            MinecraftVersion::year_release(major_number, minor_number, patch_number).into_id(),
        );
    }

    Some(MinecraftVersion::legacy_release(1, major_number, minor_number).into_id())
}

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

    #[tokio::test]
    async fn public_driver_inspection_uses_family_identity_rules() {
        use crate::{
            driver::Driver,
            families::version_json::{BaseInstanceLayout, BaseRootLayout},
            inspect::InstanceProbe,
        };

        let driver = NeoForgeDriver::with_defaults();
        for (id, expected) in [
            ("custom-instance", None),
            ("1.21.1-neoforge-21.1.265", Some("21.1.265")),
            ("1.20.1-neoforge-21.1.265", None),
        ] {
            let probe = InstanceProbe {
                metadata: Some(metadata(
                    id,
                    &["net.neoforged.fancymodloader:loader:4.0.45"],
                )),
            };
            let installed = Driver::<BaseRootLayout, BaseInstanceLayout>::inspect(&driver, &probe)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(installed.driver.id, "neoforge");
            assert_eq!(installed.driver_version.as_deref(), expected);
        }
    }
}
