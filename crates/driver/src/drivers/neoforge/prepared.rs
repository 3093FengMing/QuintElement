use anyhow::Result;
use elemental_schema::forge::ForgeInstallerProfile;

use crate::{
    drivers::neoforge::source::{NeoForgeEndpoints, NeoForgeSource},
    families::installer::{
        InstallerFamily, InstallerFamilyInstallStatus, InstallerFamilyRemoteResolver,
        PreparedInstallerFamilyLaunchVersion, PreparedInstallerFamilyVersion,
        ResolvedInstallerFamilyLaunchVersion, ResolvedInstallerFamilyMetadata,
        ResolvedInstallerFamilyVersion, profile_game_and_raw_loader_version,
    },
    loader_version::LoaderVersionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NeoForgeFamily;

pub type NeoForgeInstallStatus = InstallerFamilyInstallStatus;
pub type NeoForgeRemoteResolver = InstallerFamilyRemoteResolver<NeoForgeFamily>;
pub type ResolvedNeoForgeMetadata = ResolvedInstallerFamilyMetadata<NeoForgeFamily>;
pub type ResolvedNeoForgeLaunchVersion<L, VL> =
    ResolvedInstallerFamilyLaunchVersion<NeoForgeFamily, L, VL>;
pub type PreparedNeoForgeLaunchVersion<L, VL> =
    PreparedInstallerFamilyLaunchVersion<NeoForgeFamily, L, VL>;
pub type ResolvedNeoForgeVersion<L, VL> = ResolvedInstallerFamilyVersion<NeoForgeFamily, L, VL>;
pub type PreparedNeoForgeVersion<L, VL> = PreparedInstallerFamilyVersion<NeoForgeFamily, L, VL>;

impl InstallerFamily for NeoForgeFamily {
    type Source = NeoForgeSource;
    type Endpoints = NeoForgeEndpoints;

    const FAMILY_NAME: &'static str = "neoforge";

    fn profile_identity(
        install_profile: &ForgeInstallerProfile,
    ) -> Result<(String, LoaderVersionId)> {
        let (game_version, raw_version) =
            profile_game_and_raw_loader_version(install_profile, Self::FAMILY_NAME, "neoforge")?;
        // Modern installers use a display id while Maven `path` already holds
        // the bare release version. Strip only this family's exact game prefix.
        let prefix = format!("{game_version}-neoforge-");
        let loader_version = raw_version
            .as_str()
            .strip_prefix(&prefix)
            .or_else(|| raw_version.as_str().strip_prefix("neoforge-"))
            .unwrap_or(raw_version.as_str());
        Ok((game_version, LoaderVersionId::from(loader_version)))
    }
}
