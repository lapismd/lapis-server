use super::{network_policy as policy, *};
use reqwest::{
    Method,
    header::{HeaderName, HeaderValue},
};
use serde_json::{Value, json};
use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

impl broker::Broker {
    pub(super) fn network(
        &self,
        input: Value,
        grants: &[JobNetworkGrant],
        deadline: Instant,
    ) -> Result<Value> {
        let request: policy::Request =
            serde_json::from_value(input).map_err(|_| invalid("Invalid network request"))?;
        let (url, method, grant, mut headers) = policy::grant(&request, grants)?;
        let host = url
            .host_str()
            .ok_or_else(Error::denied)?
            .trim_matches(['[', ']']);
        let port = url.port_or_known_default().ok_or_else(Error::denied)?;
        let mut mutation_started = false;
        let result = self.owner.executor.handle.block_on(async {
            let addresses: Vec<_> = if let Ok(ip) = host.parse::<IpAddr>() {
                vec![SocketAddr::new(ip, port)]
            } else {
                let mut builder = hickory_resolver::Resolver::builder_tokio()
                    .map_err(|_| invalid("Job network DNS unavailable"))?;
                builder.options_mut().ip_strategy =
                    hickory_resolver::config::LookupIpStrategy::Ipv4AndIpv6;
                let resolver = builder
                    .build()
                    .map_err(|_| invalid("Job network DNS unavailable"))?;
                // A fully qualified name prevents system search suffixes from
                // widening an approved origin. Resolution uses async sockets,
                // never Tokio's uncancellable blocking OS resolver.
                self.network_wait(resolver.lookup_ip(absolute_dns_name(host)), deadline)
                    .await?
                    .map_err(|_| invalid("Job network DNS failed"))?
                    .iter()
                    .take(33)
                    .map(|ip| SocketAddr::new(ip, port))
                    .collect()
            };
            if addresses.is_empty()
                || addresses.len() > 32
                || addresses.iter().any(|a| {
                    !policy::address(a.ip(), grant.allow_private_network, url.scheme() == "https")
                })
            {
                return Err(Error::denied());
            }
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .referer(false)
                .timeout(deadline.saturating_duration_since(Instant::now()))
                .connect_timeout(Duration::from_secs(10))
                .resolve_to_addrs(host, &addresses)
                .build()
                .map_err(|_| invalid("Job network transport unavailable"))?;
            let mut injected = None;
            if let Some(id) = &request.credential {
                let binding = grant
                    .credentials
                    .iter()
                    .find(|binding| &binding.id == id)
                    .ok_or_else(Error::denied)?;
                let secret = self
                    .credential_store()?
                    .get(
                        &crate::credentials::SecretName::new(id.clone())
                            .map_err(|_| Error::denied())?,
                    )
                    .map_err(|_| invalid("Job credential unavailable"))?
                    .ok_or_else(|| invalid("Job credential is not configured"))?;
                let mut bytes = zeroize::Zeroizing::new(binding.prefix.as_bytes().to_vec());
                if secret.expose_secret().is_empty() || secret.expose_secret().len() > 8192 {
                    return Err(invalid("Job credential exceeds network header bounds"));
                }
                bytes.extend_from_slice(secret.expose_secret());
                let mut value = HeaderValue::from_bytes(&bytes)
                    .map_err(|_| invalid("Job credential is not a valid header"))?;
                value.set_sensitive(true);
                headers.insert(
                    HeaderName::from_bytes(binding.header.as_bytes())
                        .map_err(|_| Error::denied())?,
                    value,
                );
                injected = Some(secret);
            }
            self.check_admission()?;
            let mutation = !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
            let mut send = client.request(method, url.clone()).headers(headers);
            if let Some(body) = request.body {
                send = send.body(body);
            }
            if mutation {
                mutation_started = true;
                self.external_effect.store(true, Ordering::Release);
            }
            // A request may have reached the peer once send is first polled.
            let mut response = self
                .network_wait(send.send(), deadline)
                .await?
                .map_err(|_| invalid("Job network request failed"))?;
            if response.status().is_redirection() {
                return Err(invalid("Job network redirects are not allowed"));
            }
            if response
                .content_length()
                .is_some_and(|size| size > policy::MAX_BODY as u64)
            {
                return Err(invalid("Job network response exceeds bounds"));
            }
            let status = response.status().as_u16();
            let mut headers = serde_json::Map::new();
            for name in [
                "content-type",
                "etag",
                "last-modified",
                "retry-after",
                "x-request-id",
            ] {
                if let Some(value) = response.headers().get(name).and_then(|v| v.to_str().ok()) {
                    reject_reflection(value.as_bytes(), injected.as_ref())?;
                    if value.len() > 1024 {
                        return Err(invalid("Job response header exceeds bounds"));
                    }
                    headers.insert(name.into(), json!(value));
                }
            }
            let mut body = Vec::new();
            while let Some(chunk) = self
                .network_wait(response.chunk(), deadline)
                .await?
                .map_err(|_| invalid("Job network response failed"))?
            {
                if body.len() + chunk.len() > policy::MAX_BODY {
                    return Err(invalid("Job network response exceeds bounds"));
                }
                body.extend_from_slice(&chunk);
            }
            self.check_admission()?;
            reject_reflection(&body, injected.as_ref())?;
            let body = String::from_utf8(body)
                .map_err(|_| invalid("Job network response must be UTF-8"))?;
            Ok(json!({"status":status,"body":body,"headers":headers}))
        });
        if result.is_err() && mutation_started {
            self.external_uncertain.store(true, Ordering::Release);
        }
        result
    }

    async fn network_wait<F: Future>(&self, future: F, deadline: Instant) -> Result<F::Output> {
        network_wait(future, deadline, || self.check_admission()).await
    }
}

async fn network_wait<F: Future>(
    future: F,
    deadline: Instant,
    check_admission: impl Fn() -> Result<()>,
) -> Result<F::Output> {
    tokio::pin!(future);
    loop {
        check_admission()?;
        if Instant::now() >= deadline {
            return Err(invalid("Job network deadline expired"));
        }
        tokio::select! {
            value = &mut future => return Ok(value),
            _ = tokio::time::sleep(Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now()))) => {}
        }
    }
}

fn absolute_dns_name(host: &str) -> String {
    if host.ends_with('.') {
        host.to_owned()
    } else {
        format!("{host}.")
    }
}

#[cfg(test)]
mod tests;

fn reject_reflection(bytes: &[u8], secret: Option<&crate::credentials::SecretBytes>) -> Result<()> {
    if let Some(secret) = secret {
        let secret = secret.expose_secret();
        if bytes.windows(secret.len()).any(|window| window == secret) {
            return Err(invalid("Job endpoint reflected a protected credential"));
        }
    }
    Ok(())
}
