use elemental_core::minecraft::MinecraftVersionId;
use elemental_schema::forge::ForgeInstallerProfile;

use crate::{
    catalog::{InstallerMavenCatalog, LoaderRelease},
    descriptors::FORGE_DRIVER,
    error::{Error, Result},
    families::installer::{
        InstallerFamily, InstallerFamilyDriver, InstallerMavenSource, MavenEndpoints, MavenSpec,
        profile_game_and_raw_loader_version,
    },
    inspect::{LibraryPrefixSet, ProfileIdPattern, ProfiledDriverIdentity},
    loader_version::LoaderVersionId,
    url::Origin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForgeOrigin;

impl Origin for ForgeOrigin {
    fn canonical(self) -> &'static str {
        "https://maven.minecraftforge.net"
    }

    fn all() -> &'static [Self] {
        &[Self]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ForgeSpec;

impl MavenSpec for ForgeSpec {
    type Origin = ForgeOrigin;

    const SOURCE_NAME: &'static str = "forge source";
    const MAVEN_ORIGIN: ForgeOrigin = ForgeOrigin;
    const GROUP: &'static str = "net.minecraftforge";
    const ARTIFACT: &'static str = "forge";

    fn release(version: String) -> Option<(MinecraftVersionId, LoaderRelease)> {
        let mut parts = version.split('-');
        let game_version = parts.next()?;
        let loader_version = parts.next()?;
        let game_version = MinecraftVersionId::from(game_version);
        Some((
            game_version.clone(),
            LoaderRelease::new(game_version, LoaderVersionId::from(loader_version)),
        ))
    }
}

pub type ForgeEndpoints = MavenEndpoints<ForgeSpec>;
pub type ForgeSource = InstallerMavenSource<ForgeEndpoints>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForgeFamily;

impl InstallerFamily for ForgeFamily {
    type Source = ForgeSource;
    type Endpoints = ForgeEndpoints;

    const IDENTITY: ProfiledDriverIdentity = FORGE_IDENTITY;
    const FAMILY_NAME: &'static str = "forge";
    const LOADER_VERSION_PREFIX: &'static str = "forge-";

    fn profile_identity(
        install_profile: &ForgeInstallerProfile,
    ) -> Result<(String, LoaderVersionId)> {
        let (game_version, raw_version) =
            profile_game_and_raw_loader_version(install_profile, Self::FAMILY_NAME, "forge")?;
        let (coordinate_game, loader_version) =
            raw_version
                .as_str()
                .split_once('-')
                .ok_or_else(|| Error::InvalidNotation {
                    label: "forge installer version".to_owned(),
                    notation: raw_version.to_string(),
                })?;
        if coordinate_game != game_version {
            return Err(Error::InstallerGameVersionMismatch {
                family_name: Self::FAMILY_NAME.to_owned(),
                actual: coordinate_game.to_owned(),
                expected: game_version,
            });
        }
        let loader_version = loader_version
            .strip_prefix(Self::LOADER_VERSION_PREFIX)
            .unwrap_or(loader_version);
        Ok((game_version, LoaderVersionId::from(loader_version)))
    }
}

pub type ForgeDriver = InstallerFamilyDriver<ForgeFamily>;

const FORGE_LIBRARIES: LibraryPrefixSet = LibraryPrefixSet::new(&["net.minecraftforge:fmlloader:"]);
const FORGE_IDENTITY: ProfiledDriverIdentity = ProfiledDriverIdentity::new(
    FORGE_DRIVER,
    FORGE_LIBRARIES,
    FORGE_LIBRARIES,
    ProfileIdPattern::new("-forge-"),
);

pub type ForgeCatalog = InstallerMavenCatalog<ForgeEndpoints>;

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn profile_identity_accepts_coordinate_and_display_versions() {
        for profile in [
            serde_json::json!({"minecraft": "1.20.1", "path": "net.minecraftforge:forge:1.20.1-47.4.28"}),
            serde_json::json!({"minecraft": "1.20.1", "version": "1.20.1-forge-47.4.28"}),
            serde_json::json!({"install": {"minecraft": "1.20.1", "path": "net.minecraftforge:forge:1.20.1-47.4.28"}}),
        ] {
            let profile: ForgeInstallerProfile = serde_json::from_value(profile).unwrap();
            let (game_version, loader_version) = ForgeFamily::profile_identity(&profile).unwrap();
            assert_eq!(game_version, "1.20.1");
            assert_eq!(loader_version.as_str(), "47.4.28");
        }
    }

    #[test]
    fn profile_identity_rejects_a_conflicting_coordinate_game() {
        for profile in [
            serde_json::json!({"minecraft": "1.20.1", "path": "net.minecraftforge:forge:1.21.1-52.0.7"}),
            serde_json::json!({"minecraft": "1.20.1", "path": "net.minecraftforge:forge:1.21.1-forge-47.4.28"}),
            serde_json::json!({"minecraft": "1.20.1", "version": "1.21.1-forge-47.4.28"}),
        ] {
            let profile: ForgeInstallerProfile = serde_json::from_value(profile).unwrap();
            assert!(matches!(
                ForgeFamily::profile_identity(&profile),
                Err(Error::InstallerGameVersionMismatch { actual, expected, .. })
                    if actual == "1.21.1" && expected == "1.20.1"
            ));
        }
    }
}
