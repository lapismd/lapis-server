use super::*;
use reqwest::{
    Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::Deserialize;
use std::{collections::BTreeMap, net::IpAddr};
#[cfg(test)]
mod tests;

pub(super) const MAX_BODY: usize = 128 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Request {
    pub url: String,
    #[serde(default = "get")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub credential: Option<String>,
}
fn get() -> String {
    "GET".into()
}

pub(super) fn url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| invalid("Invalid job network URL"))?;
    if value.len() > 4096
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
        || !matches!(url.scheme(), "https" | "http")
    {
        return Err(invalid("Invalid job network URL"));
    }
    Ok(url)
}

pub(super) fn validate(grants: &[JobNetworkGrant]) -> Result<()> {
    if grants.len() > 8 {
        return Err(invalid("Too many network origins"));
    }
    let mut origins = std::collections::HashSet::new();
    for grant in grants {
        let target = url(&grant.origin)?;
        if grant.origin != target.origin().ascii_serialization()
            || !origins.insert(&grant.origin)
            || (target.scheme() != "https" && !grant.allow_private_network)
            || grant.methods.is_empty()
            || grant.methods.len() > 7
            || grant.credentials.len() > 8
        {
            return Err(invalid("Invalid network grant"));
        }
        let mut methods = std::collections::HashSet::new();
        for method in &grant.methods {
            if !matches!(
                method.as_str(),
                "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
            ) || !methods.insert(method)
            {
                return Err(invalid("Invalid network method grant"));
            }
        }
        let mut ids = std::collections::HashSet::new();
        for binding in &grant.credentials {
            if !slot(&binding.id)
                || !ids.insert(&binding.id)
                || !matches!(binding.header.as_str(), "authorization" | "x-api-key")
                || !matches!(binding.prefix.as_str(), "" | "Bearer " | "Basic ")
            {
                return Err(invalid("Invalid network credential binding"));
            }
        }
    }
    Ok(())
}

pub(super) fn slot(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_lowercase() || b.is_ascii_digit() || (i > 0 && b == b'-'))
}

pub(super) fn grant<'a>(
    request: &Request,
    grants: &'a [JobNetworkGrant],
) -> Result<(Url, Method, &'a JobNetworkGrant, HeaderMap)> {
    validate(grants)?;
    let url = url(&request.url)?;
    let grant = grants
        .iter()
        .find(|g| {
            g.origin == url.origin().ascii_serialization() && g.methods.contains(&request.method)
        })
        .ok_or_else(Error::denied)?;
    let method = Method::from_bytes(request.method.as_bytes()).map_err(|_| Error::denied())?;
    if request.body.as_ref().is_some_and(|body| body.len() > 65536)
        || request.headers.len() > 16
        || (matches!(method, Method::GET | Method::HEAD) && request.body.is_some())
    {
        return Err(invalid("Job network request exceeds bounds"));
    }
    let mut headers = HeaderMap::new();
    let mut bytes = 0;
    for (name, value) in &request.headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| invalid("Invalid network header"))?;
        // Do not admit ambient cookies, proxy credentials, alternate routing or framing.
        if !matches!(
            name.as_str(),
            "accept" | "content-type" | "if-none-match" | "if-modified-since" | "idempotency-key"
        ) {
            return Err(Error::denied());
        }
        bytes += name.as_str().len() + value.len();
        if bytes > 8192 {
            return Err(invalid("Job network headers exceed bounds"));
        }
        let value = HeaderValue::from_str(value).map_err(|_| invalid("Invalid network header"))?;
        if headers.insert(name, value).is_some() {
            return Err(invalid("Duplicate network header"));
        }
    }
    if let Some(id) = &request.credential
        && !grant.credentials.iter().any(|binding| &binding.id == id)
    {
        return Err(Error::denied());
    }
    Ok((url, method, grant, headers))
}

/// Conservative address policy. A private grant never admits link-local/metadata,
/// multicast, benchmarking, documentation, translation or other special-use ranges.
pub(super) fn address(ip: IpAddr, private: bool, https: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            if ip.octets() == [168, 63, 129, 16] {
                return false;
            }
            let local = ip.is_loopback() || ip.is_private();
            if local {
                return private;
            }
            https
                && a != 0
                && a != 10
                && a != 127
                && a < 224
                && !(a == 100 && (64..=127).contains(&b))
                && !(a == 169 && b == 254)
                && !(a == 172 && (16..=31).contains(&b))
                && !(a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                && !(a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                && !(a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return address(IpAddr::V4(v4), private, https);
            }
            let [a, b, ..] = ip.segments();
            if ip
                == "fd00:ec2::254"
                    .parse::<std::net::Ipv6Addr>()
                    .expect("Metadata address")
                || ip
                    == "fd20:ce::254"
                        .parse::<std::net::Ipv6Addr>()
                        .expect("Metadata address")
            {
                return false;
            }
            if ip.is_loopback() || a & 0xfe00 == 0xfc00 {
                return private;
            }
            https
                && a & 0xe000 == 0x2000
                && a != 0x2002
                && !(a == 0x2001 && (b < 0x0200 || b == 0x0db8))
                && !(a == 0x3fff && b < 0x1000)
        }
    }
}
