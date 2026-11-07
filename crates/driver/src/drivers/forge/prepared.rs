use elemental_schema::forge::ForgeInstallerProfile;

use crate::{
    drivers::forge::source::{ForgeEndpoints, ForgeSource, parse_installer_version},
    error::{Error, Result},
    families::installer::{
        InstallerFamily, InstallerFamilyInstallStatus, InstallerFamilyRemoteResolver,
        PreparedInstallerFamilyLaunchVersion, PreparedInstallerFamilyVersion,
        ResolvedInstallerFamilyLaunchVersion, ResolvedInstallerFamilyMetadata,
        ResolvedInstallerFamilyVersion, profile_game_and_raw_loader_version,
    },
    loader_version::LoaderVersionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForgeFamily;

pub type ForgeInstallStatus = InstallerFamilyInstallStatus;
pub type ForgeRemoteResolver = InstallerFamilyRemoteResolver<ForgeFamily>;
pub type ResolvedForgeMetadata = ResolvedInstallerFamilyMetadata<ForgeFamily>;
pub type ResolvedForgeLaunchVersion<L, VL> =
    ResolvedInstallerFamilyLaunchVersion<ForgeFamily, L, VL>;
pub type PreparedForgeLaunchVersion<L, VL> =
    PreparedInstallerFamilyLaunchVersion<ForgeFamily, L, VL>;
pub type ResolvedForgeVersion<L, VL> = ResolvedInstallerFamilyVersion<ForgeFamily, L, VL>;
pub type PreparedForgeVersion<L, VL> = PreparedInstallerFamilyVersion<ForgeFamily, L, VL>;

impl InstallerFamily for ForgeFamily {
    type Source = ForgeSource;
    type Endpoints = ForgeEndpoints;

    const FAMILY_NAME: &'static str = "forge";
    const LOADER_VERSION_PREFIX: &'static str = "forge-";

    fn profile_identity(
        install_profile: &ForgeInstallerProfile,
    ) -> Result<(String, LoaderVersionId)> {
        let (game_version, raw_version) =
            profile_game_and_raw_loader_version(install_profile, Self::FAMILY_NAME, "forge")?;
        let (coordinate_game, loader_version) = parse_installer_version(raw_version.as_str())?;
        if coordinate_game != game_version {
            return Err(Error::InstallerGameVersionMismatch {
                family_name: Self::FAMILY_NAME.to_owned(),
                actual: coordinate_game,
                expected: game_version,
            });
        }
        Ok((game_version, LoaderVersionId::from(loader_version)))
    }
}

#[cfg(test)]
mod tests {
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
        let profile: ForgeInstallerProfile = serde_json::from_value(serde_json::json!({
            "minecraft": "1.20.1", "path": "net.minecraftforge:forge:1.21.1-52.0.7"
        }))
        .unwrap();
        assert!(matches!(
            ForgeFamily::profile_identity(&profile),
            Err(Error::InstallerGameVersionMismatch { actual, expected, .. })
                if actual == "1.21.1" && expected == "1.20.1"
        ));
    }
}
