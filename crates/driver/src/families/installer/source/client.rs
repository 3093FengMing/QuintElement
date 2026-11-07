use elemental_core::{minecraft::MinecraftVersionId, storage::Storage};
use elemental_schema::maven::MavenMetadataBody;

use crate::{
    error::{Error, Result},
    families::{
        installer::{
            InstallerArtifact, InstallerArtifactSource, InstallerMavenEndpoints,
            build_installer_artifact,
        },
        version_json::VersionJsonRootLayout,
    },
    http::{HttpSource, ResourceLimits},
    loader_version::LoaderVersionId,
    maven::fetch_maven_metadata,
};

#[derive(Debug, Clone)]
pub struct InstallerMavenSource<E>
where
    E: InstallerMavenEndpoints,
{
    inner: HttpSource<E>,
}

impl<E> Default for InstallerMavenSource<E>
where
    E: InstallerMavenEndpoints + Default,
{
    fn default() -> Self {
        Self::new(E::default())
    }
}

impl<E> InstallerMavenSource<E>
where
    E: InstallerMavenEndpoints,
{
    pub fn new(endpoints: E) -> Self {
        Self {
            inner: HttpSource::new(endpoints, E::SOURCE_NAME),
        }
    }

    pub fn with_client(endpoints: E, client: reqwest::Client) -> Self {
        Self {
            inner: HttpSource::with_client(endpoints, client),
        }
    }

    pub fn endpoints(&self) -> &E {
        self.inner.endpoints()
    }

    /// Apply response-size and total-request bounds to Maven metadata requests.
    /// Installer artifact downloads continue to use the download session's policy.
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.inner = self.inner.with_resource_limits(limits);
        self
    }

    pub async fn maven_metadata(&self) -> Result<MavenMetadataBody> {
        let url = self.endpoints().maven_metadata_url()?;
        let limits = self.inner.resource_limits();
        if limits.max_response_bytes.is_none() && limits.request_timeout.is_none() {
            return fetch_maven_metadata(self.inner.client()?, url, E::SOURCE_NAME).await;
        }
        let raw = self.inner.fetch_text(&url, E::SOURCE_NAME).await?;
        quick_xml::de::from_str(&raw).map_err(|source| Error::DecodeMavenMetadata {
            source_label: E::SOURCE_NAME.to_owned(),
            url,
            source,
        })
    }

    pub fn installer_artifact<L>(
        &self,
        game_storage: &Storage<L>,
        game_version: &MinecraftVersionId,
        loader_version: &LoaderVersionId,
    ) -> Result<InstallerArtifact>
    where
        L: VersionJsonRootLayout,
    {
        let spec = self
            .endpoints()
            .installer_artifact_spec(game_version, loader_version)?;

        build_installer_artifact(
            game_storage,
            spec.coordinate,
            spec.download_url,
            spec.relative_path,
        )
    }
}

impl<E> InstallerArtifactSource for InstallerMavenSource<E>
where
    E: InstallerMavenEndpoints,
{
    type Endpoints = E;

    fn endpoints(&self) -> &Self::Endpoints {
        InstallerMavenSource::endpoints(self)
    }

    fn installer_artifact<L>(
        &self,
        game_storage: &Storage<L>,
        game_version: &MinecraftVersionId,
        loader_version: &LoaderVersionId,
    ) -> Result<InstallerArtifact>
    where
        L: VersionJsonRootLayout,
    {
        InstallerMavenSource::installer_artifact(self, game_storage, game_version, loader_version)
    }
}
