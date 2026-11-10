use crate::{
    cache::{Cache, MAX_BLOB, verified},
    digest, log,
};
use anyhow::{Context, Result, bail, ensure};
use oci_client::{
    Reference,
    client::{Client, ClientConfig, ClientProtocol},
    manifest::{OciDescriptor, OciManifest},
    secrets::RegistryAuth,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
};
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub struct Registry {
    client: Client,
    architecture: String,
    auth_override: Option<RegistryAuth>,
}
#[derive(Serialize, Deserialize)]
struct Resolution {
    top: String,
}
pub struct Artifact {
    pub manifest: String,
    pub layer: String,
    pub media: String,
    pub blob: PathBuf,
    pub platform: String,
}
impl Registry {
    pub fn new(config: ClientConfig, architecture: &str) -> Result<Self> {
        ensure!(
            matches!(architecture, "amd64" | "arm64"),
            "unsupported Mod architecture"
        );
        Ok(Self {
            client: Client::try_from(config).context("OCI client initialization failed; install CA certificates or configure a trust bundle")?,
            architecture: architecture.into(),
            auth_override: None,
        })
    }
    pub fn with_auth(mut self, auth: RegistryAuth) -> Self {
        self.auth_override = Some(auth);
        self
    }
    pub fn production() -> Result<Self> {
        let exceptions = std::env::var("DOCKER_MODS_INSECURE_REGISTRIES")
            .unwrap_or_default()
            .split(',')
            .filter(|host| !host.is_empty())
            .map(str::to_owned)
            .collect();
        Self::new(
            ClientConfig {
                protocol: ClientProtocol::HttpsExcept(exceptions),
                connect_timeout: Some(Duration::from_secs(15)),
                read_timeout: Some(Duration::from_secs(60)),
                ..Default::default()
            },
            match std::env::consts::ARCH {
                "x86_64" => "amd64",
                "aarch64" => "arm64",
                _ => bail!("unsupported Mod architecture"),
            },
        )
    }
    pub async fn resolve(
        &self,
        reference: &Reference,
        cache: &Cache,
        offline: bool,
    ) -> Result<Artifact> {
        let auth = if offline {
            RegistryAuth::Anonymous
        } else if let Some(auth) = &self.auth_override {
            auth.clone()
        } else {
            crate::credentials::load(reference)?
        };
        let resolution = cache.resolution(&reference.to_string());
        let initial = if offline {
            let record: Resolution = serde_json::from_slice(&fs::read(&resolution)?)?;
            if let Some(expected) = reference.digest() {
                ensure!(record.top == expected, "pinned reference cache mismatch");
            }
            record.top
        } else {
            self.manifest(reference, &auth, cache).await?
        };
        let mut selected = initial.clone();
        for depth in 0..3 {
            let bytes = self.cached_bytes(cache, &selected, 4 * 1024 * 1024)?;
            let manifest: OciManifest = serde_json::from_slice(&bytes)?;
            match manifest {
                OciManifest::Image(image) => {
                    ensure!(
                        image.schema_version == 2 && image.layers.len() == 1,
                        "Mods require schema 2 and exactly one layer"
                    );
                    self.blob(reference, &image.config, &auth, cache, offline, 1024 * 1024)
                        .await?;
                    let config: serde_json::Value = serde_json::from_slice(&self.cached_bytes(
                        cache,
                        &image.config.digest,
                        1024 * 1024,
                    )?)?;
                    ensure!(
                        config["os"] == "linux" && config["architecture"] == self.architecture,
                        "unsupported Mod platform"
                    );
                    let layer = &image.layers[0];
                    ensure!(
                        matches!(
                            layer.media_type.as_str(),
                            "application/vnd.oci.image.layer.v1.tar"
                                | "application/vnd.oci.image.layer.v1.tar+gzip"
                                | "application/vnd.docker.image.rootfs.diff.tar.gzip"
                        ),
                        "unsupported Mod layer media type"
                    );
                    self.blob(reference, layer, &auth, cache, offline, MAX_BLOB)
                        .await?;
                    if !offline {
                        cache.write(
                            &resolution,
                            &serde_json::to_vec(&Resolution { top: initial })?,
                        )?;
                    }
                    return Ok(Artifact {
                        manifest: selected,
                        layer: layer.digest.clone(),
                        media: layer.media_type.clone(),
                        blob: cache.blob(&layer.digest)?,
                        platform: format!("linux/{}", self.architecture),
                    });
                }
                OciManifest::ImageIndex(index) => {
                    ensure!(depth < 2, "nested Mod index limit");
                    let entry = index
                        .manifests
                        .iter()
                        .find(|entry| {
                            entry.platform.as_ref().is_some_and(|p| {
                                p.os.to_string() == "linux"
                                    && p.architecture.to_string() == self.architecture
                            })
                        })
                        .context("unsupported Mod architecture")?;
                    selected = entry.digest.clone();
                    if offline {
                        self.cached_bytes(cache, &selected, 4 * 1024 * 1024)?;
                    } else {
                        let pinned: Reference = format!(
                            "{}/{}@{}",
                            reference.registry(),
                            reference.repository(),
                            selected
                        )
                        .parse()?;
                        ensure!(
                            self.manifest(&pinned, &auth, cache).await? == selected,
                            "index manifest digest mismatch"
                        );
                    }
                }
            }
        }
        bail!("Mod index resolution failed")
    }
    fn cached_bytes(&self, cache: &Cache, digest: &str, max: u64) -> Result<Vec<u8>> {
        let path = cache.blob(digest)?;
        verified(&path, digest, max)?;
        Ok(fs::read(path)?)
    }
    async fn manifest(
        &self,
        reference: &Reference,
        auth: &RegistryAuth,
        cache: &Cache,
    ) -> Result<String> {
        let (bytes, expected) = self
            .client
            .pull_manifest_raw(
                reference,
                auth,
                &[
                    "application/vnd.docker.distribution.manifest.v2+json",
                    "application/vnd.docker.distribution.manifest.list.v2+json",
                    "application/vnd.oci.image.manifest.v1+json",
                    "application/vnd.oci.image.index.v1+json",
                ],
            )
            .await
            .context("Mod manifest request failed")?;
        ensure!(
            bytes.len() <= 4 * 1024 * 1024 && digest(&bytes) == expected,
            "manifest size/digest mismatch"
        );
        if let Some(pinned) = reference.digest() {
            ensure!(expected == pinned, "pinned manifest digest mismatch");
        }
        cache.write(&cache.blob(&expected)?, &bytes)?;
        Ok(expected)
    }
    async fn blob(
        &self,
        reference: &Reference,
        descriptor: &OciDescriptor,
        _auth: &RegistryAuth,
        cache: &Cache,
        offline: bool,
        max: u64,
    ) -> Result<()> {
        ensure!(
            descriptor.size >= 0 && descriptor.size as u64 <= max,
            "Mod blob exceeds size limit"
        );
        let path = cache.blob(&descriptor.digest)?;
        if path.exists() {
            verified(&path, &descriptor.digest, max)?;
            ensure!(
                fs::metadata(&path)?.len() == descriptor.size as u64,
                "cached blob size mismatch"
            );
            log("cache-hit", &descriptor.digest);
            return Ok(());
        }
        ensure!(!offline, "offline Mod blob missing");
        let temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        let file = tokio::fs::File::from_std(temporary.reopen()?);
        let mut writer = LimitedWriter {
            file,
            remaining: descriptor.size as u64,
        };
        self.client
            .pull_blob(reference, descriptor, &mut writer)
            .await
            .context("Mod blob request failed")?;
        writer.flush().await?;
        ensure!(writer.remaining == 0, "Mod blob size mismatch");
        drop(writer);
        verified(temporary.path(), &descriptor.digest, max)?;
        temporary.persist(&path)?;
        log("downloaded", &descriptor.digest);
        Ok(())
    }
}

struct LimitedWriter {
    file: tokio::fs::File,
    remaining: u64,
}
impl AsyncWrite for LimitedWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if bytes.len() as u64 > self.remaining {
            return Poll::Ready(Err(std::io::Error::other(
                "Mod blob exceeded declared size",
            )));
        }
        match Pin::new(&mut self.file).poll_write(cx, bytes) {
            Poll::Ready(Ok(count)) => {
                self.remaining -= count as u64;
                Poll::Ready(Ok(count))
            }
            other => other,
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_shutdown(cx)
    }
}
