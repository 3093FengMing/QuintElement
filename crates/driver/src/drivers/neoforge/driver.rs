use elemental_schema::mojang::piston::PistonMetaData;

use crate::{
    descriptors::NEOFORGE_DRIVER,
    driver::InstalledDriver,
    drivers::neoforge::prepared::NeoForgeFamily,
    families::installer::{
        InstallerFamilyDriver, InstallerFamilyDriverSpec, LaunchedInstallerFamilyVersion,
    },
    inspect::LibraryPrefixSet,
};

pub type NeoForgeDriver = InstallerFamilyDriver<NeoForgeFamily>;
pub type LaunchedNeoForgeVersion<L, VL> = LaunchedInstallerFamilyVersion<NeoForgeFamily, L, VL>;

impl InstallerFamilyDriverSpec for NeoForgeFamily {
    const DRIVER: crate::driver::DriverDescriptor = NEOFORGE_DRIVER;

    const INSPECT_PREFIXES: &'static [&'static str] = &[
        "net.neoforged:neoforge:",
        "net.neoforged:forge:",
        "net.neoforged:fmlloader:",
        "net.neoforged.fancymodloader:",
    ];

    fn inspect_installed(metadata: &PistonMetaData) -> Option<InstalledDriver> {
        if !LibraryPrefixSet::new(Self::INSPECT_PREFIXES).matches(metadata) {
            return None;
        }
        let driver_version = LibraryPrefixSet::new(&[
            "net.neoforged:neoforge:",
            "net.neoforged:forge:",
            "net.neoforged:fmlloader:",
        ])
        .version(metadata)
        .or_else(|| {
            let game_prefix = format!(
                "{}-neoforge-",
                metadata.inherits_from.as_deref().unwrap_or_default()
            );
            metadata
                .id
                .strip_prefix("neoforge-")
                .or_else(|| metadata.id.strip_prefix(&game_prefix))
                .filter(|version| !version.is_empty())
                .map(str::to_owned)
        });
        // Modern profiles contain only FML component libraries, whose version
        // is independent of the NeoForge release. Keep unknown releases unknown.
        Some(InstalledDriver::version_json(
            Self::DRIVER,
            metadata,
            driver_version,
        ))
    }
}