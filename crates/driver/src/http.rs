use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// Optional bounds for metadata HTTP requests, including reading the response body.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceLimits {
    /// Maximum decoded response size, including responses without a Content-Length.
    pub max_response_bytes: Option<usize>,
    /// Total request time, from connecting until the response body has been read.
    pub request_timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct HttpSource<E> {
    client: std::result::Result<reqwest::Client, String>,
    endpoints: E,
    limits: ResourceLimits,
}

impl<E> HttpSource<E> {
    pub fn new(endpoints: E, source_name: &str) -> Self {
        Self {
            client: build_default_client(source_name),
            endpoints,
            limits: ResourceLimits::default(),
        }
    }

    pub fn with_client(endpoints: E, client: reqwest::Client) -> Self {
        Self {
            client: Ok(client),
            endpoints,
            limits: ResourceLimits::default(),
        }
    }

    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn resource_limits(&self) -> ResourceLimits {
        self.limits
    }

    pub async fn fetch_json<T: DeserializeOwned>(&self, url: &str, name: &str) -> Result<T> {
        if self.limits.max_response_bytes.is_none() && self.limits.request_timeout.is_none() {
            return fetch_json(self.client()?, url, name).await;
        }
        let body = fetch_bytes_with_limits(self.client()?, url, name, self.limits).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub async fn fetch_text(&self, url: &str, name: &str) -> Result<String> {
        if self.limits.max_response_bytes.is_none() && self.limits.request_timeout.is_none() {
            return fetch_text(self.client()?, url, name).await;
        }
        let body = fetch_bytes_with_limits(self.client()?, url, name, self.limits).await?;
        String::from_utf8(body)
            .map_err(|source| std::io::Error::new(std::io::ErrorKind::InvalidData, source).into())
    }

    pub fn client(&self) -> Result<&reqwest::Client> {
        self.client
            .as_ref()
            .map_err(|message| Error::HttpClientBuild {
                message: message.clone(),
            })
    }

    pub fn endpoints(&self) -> &E {
        &self.endpoints
    }
}

pub fn build_default_client(source_name: &str) -> std::result::Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("Elemental/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("build {source_name} client failed: {error}"))
}

pub async fn fetch_json<T>(client: &reqwest::Client, url: &str, source_name: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    send_ok(client, url, source_name)
        .await?
        .json::<T>()
        .await
        .map_err(|source| Error::DecodeResource {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })
}

pub async fn fetch_text(client: &reqwest::Client, url: &str, source_name: &str) -> Result<String> {
    send_ok(client, url, source_name)
        .await?
        .text()
        .await
        .map_err(|source| Error::DecodeResource {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })
}

pub async fn fetch_bytes(
    client: &reqwest::Client,
    url: &str,
    source_name: &str,
) -> Result<Vec<u8>> {
    send_ok(client, url, source_name)
        .await?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|source| Error::DecodeResource {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })
}

async fn send_ok(
    client: &reqwest::Client,
    url: &str,
    source_name: &str,
) -> Result<reqwest::Response> {
    client
        .get(url)
        .send()
        .await
        .map_err(|source| Error::RequestResource {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })?
        .error_for_status()
        .map_err(|source| Error::HttpStatus {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })
}

async fn fetch_bytes_with_limits(
    client: &reqwest::Client,
    url: &str,
    source_name: &str,
    limits: ResourceLimits,
) -> Result<Vec<u8>> {
    let mut request = client.get(url);
    if let Some(timeout) = limits.request_timeout {
        request = request.timeout(timeout);
    }
    let mut response = request
        .send()
        .await
        .map_err(|source| {
            if source.is_timeout() {
                Error::ResourceTimeout {
                    source_name: source_name.to_owned(),
                    url: url.to_owned(),
                    source,
                }
            } else {
                Error::RequestResource {
                    source_name: source_name.to_owned(),
                    url: url.to_owned(),
                    source,
                }
            }
        })?
        .error_for_status()
        .map_err(|source| Error::HttpStatus {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            source,
        })?;
    if let Some(limit) = limits.max_response_bytes
        && response
            .content_length()
            .is_some_and(|length| length > limit as u64)
    {
        return Err(Error::ResourceTooLarge {
            source_name: source_name.to_owned(),
            url: url.to_owned(),
            limit,
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|source| {
        if source.is_timeout() {
            Error::ResourceTimeout {
                source_name: source_name.to_owned(),
                url: url.to_owned(),
                source,
            }
        } else {
            Error::DecodeResource {
                source_name: source_name.to_owned(),
                url: url.to_owned(),
                source,
            }
        }
    })? {
        if let Some(limit) = limits.max_response_bytes
            && chunk.len() > limit.saturating_sub(body.len())
        {
            return Err(Error::ResourceTooLarge {
                source_name: source_name.to_owned(),
                url: url.to_owned(),
                limit,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    async fn read_request(connection: &mut TcpStream) {
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let length = connection.read(&mut buffer).await.unwrap();
            assert!(length > 0);
            request.extend_from_slice(&buffer[..length]);
        }
    }

    async fn response_fixture(response: &'static [u8]) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/metadata", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            read_request(&mut connection).await;
            let _ = connection.write_all(response).await;
        });
        (url, server)
    }

    fn bounded_source(limit: usize) -> HttpSource<()> {
        HttpSource::with_client((), reqwest::Client::builder().no_proxy().build().unwrap())
            .with_resource_limits(ResourceLimits {
                max_response_bytes: Some(limit),
                request_timeout: Some(Duration::from_secs(2)),
            })
    }

    #[tokio::test]
    async fn metadata_size_limit_checks_headers_and_chunked_bodies() {
        for response in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n4\r\n6789\r\n0\r\n\r\n".as_slice(),
        ] {
            let (url, server) = response_fixture(response).await;
            let error = bounded_source(8).fetch_text(&url, "catalog").await.unwrap_err();
            assert!(matches!(error, Error::ResourceTooLarge { limit: 8, .. }));
            server.await.unwrap();
        }
        let (url, server) = response_fixture(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n",
        ).await;
        assert_eq!(
            bounded_source(8).fetch_text(&url, "catalog").await.unwrap(),
            "12345678"
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn metadata_checks_http_status_before_size_or_decoding() {
        let (url, server) = response_fixture(
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 100\r\nConnection: close\r\n\r\ninvalid json",
        ).await;
        let error = bounded_source(8)
            .fetch_json::<serde_json::Value>(&url, "catalog")
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::HttpStatus { source, .. } if source.status().is_some_and(|status| status.as_u16() == 500))
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn bounded_metadata_reports_invalid_json_and_utf8() {
        let (url, server) =
            response_fixture(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{")
                .await;
        let error = bounded_source(8)
            .fetch_json::<serde_json::Value>(&url, "catalog")
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Json(_)));
        server.await.unwrap();
        let (url, server) = response_fixture(
            b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\xff",
        )
        .await;
        let error = bounded_source(8)
            .fetch_text(&url, "catalog")
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::Io(source) if source.kind() == std::io::ErrorKind::InvalidData)
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn metadata_total_timeout_covers_headers_and_streaming_body() {
        for send_headers in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/metadata", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut connection, _) = listener.accept().await.unwrap();
                read_request(&mut connection).await;
                if send_headers {
                    connection
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n")
                        .await
                        .unwrap();
                    // Activity is more frequent than read_timeout, but the full
                    // response must still stop at the request's total deadline.
                    loop {
                        if connection.write_all(b"x").await.is_err() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                } else {
                    let mut buffer = [0u8; 1];
                    let _ = connection.read(&mut buffer).await;
                }
            });
            let source = HttpSource::with_client(
                (),
                reqwest::Client::builder()
                    .no_proxy()
                    .read_timeout(Duration::from_secs(1))
                    .build()
                    .unwrap(),
            )
            .with_resource_limits(ResourceLimits {
                max_response_bytes: Some(1024),
                request_timeout: Some(Duration::from_millis(100)),
            });
            let error =
                tokio::time::timeout(Duration::from_secs(3), source.fetch_text(&url, "catalog"))
                    .await
                    .unwrap()
                    .unwrap_err();
            assert!(matches!(error, Error::ResourceTimeout { .. }));
            tokio::time::timeout(Duration::from_secs(3), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}
