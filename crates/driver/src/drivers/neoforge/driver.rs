use elemental_schema::mojang::piston::PistonMetaData;

use crate::{
    descriptors::NEOFORGE_DRIVER,
    driver::InstalledDriver,
    drivers::neoforge::prepared::NeoForgeFamily,
    families::installer::{
        InstallerFamilyDriver, InstallerFamilyDriverSpec, LaunchedInstallerFamilyVersion,
    },
    inspect::{LibraryPrefixSet, ProfileIdPattern, ProfiledDriverIdentity},
};

pub type NeoForgeDriver = InstallerFamilyDriver<NeoForgeFamily>;
pub type LaunchedNeoForgeVersion<L, VL> = LaunchedInstallerFamilyVersion<NeoForgeFamily, L, VL>;

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

impl InstallerFamilyDriverSpec for NeoForgeFamily {
    const IDENTITY: ProfiledDriverIdentity = NEOFORGE_IDENTITY;

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
        // Modern profiles contain only FML component libraries, whose version
        // is independent of the NeoForge release. Keep unknown releases unknown.
        Some(installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
