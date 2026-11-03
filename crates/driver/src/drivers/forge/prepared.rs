use anyhow::{Result, ensure};
use elemental_schema::forge::ForgeInstallerProfile;

use crate::{
    drivers::forge::source::{ForgeEndpoints, ForgeSource, parse_installer_version},
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

    fn profile_identity(
        install_profile: &ForgeInstallerProfile,
    ) -> Result<(String, LoaderVersionId)> {
        let (game_version, raw_version) =
            profile_game_and_raw_loader_version(install_profile, Self::FAMILY_NAME, "forge")?;
        let (coordinate_game, loader_version) = parse_installer_version(raw_version.as_str())?;
        ensure!(
            coordinate_game == game_version,
            "forge installer Minecraft version '{coordinate_game}' conflicts with profile version '{game_version}'"
        );
        Ok((game_version, LoaderVersionId::from(loader_version)))
    }
}
