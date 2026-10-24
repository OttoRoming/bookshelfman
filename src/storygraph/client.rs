use std::sync::Arc;
use std::time::Duration;

use url::Url;
use uuid::Uuid;
use wreq::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, OrigHeaderMap};
use wreq::redirect::Policy;
use wreq::{Client, StatusCode};
use wreq_util::{Emulation, Platform, Profile};

use super::error::{Error, Result};
use crate::cache::Cache;

const HOME_URL: &str = "https://thestorygraph.com/";
const APP_BASE_URL: &str = "https://app.thestorygraph.com";
const SESSION_COOKIE: &str = "_storygraph_marketing_session";

const HEADER_ORDER: &[&str] = &[
    "accept",
    "accept-encoding",
    "accept-language",
    "connection",
    "priority",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-user",
    "sec-gpc",
    "upgrade-insecure-requests",
    "user-agent",
];

const HEADERS: &[(&str, &str)] = &[
    (
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    ),
    ("accept-encoding", "gzip, deflate, br, zstd"),
    ("accept-language", "en-US,en;q=0.9"),
    ("connection", "keep-alive"),
    ("priority", "u=0, i"),
    ("sec-fetch-dest", "document"),
    ("sec-fetch-mode", "navigate"),
    ("sec-fetch-site", "none"),
    ("sec-fetch-user", "?1"),
    ("sec-gpc", "1"),
    ("upgrade-insecure-requests", "1"),
    (
        "user-agent",
        "Mozilla/5.0 (X11; Linux x86_64; rv:151.0) Gecko/20100101 Firefox/148.0",
    ),
];

/// A StoryGraph client.
///
/// Creating a client performs the initial handshake with StoryGraph and keeps
/// the resulting cookies for subsequent requests.
pub struct Storygraph {
    client: Client,
    headers: HeaderMap,
    header_order: OrigHeaderMap,
    cache: Arc<Cache>,
}

impl Storygraph {
    /// Creates a client using the platform's default application cache and
    /// performs the initial request needed to obtain the StoryGraph session
    /// cookie.
    pub async fn new() -> Result<Self> {
        let cache = Arc::new(Cache::open()?);

        Self::new_with_cache(cache).await
    }

    /// Creates a client using a caller-provided cache.
    ///
    /// Use this to share one [`Cache`] between several clients or with the rest
    /// of the application. [`Storygraph::new`] opens the default cache instead.
    pub async fn new_with_cache(cache: Arc<Cache>) -> Result<Self> {
        let emulation = Emulation::builder()
            .profile(Profile::Firefox148)
            .platform(Platform::Linux)
            .build();

        let client = Client::builder()
            .emulation(emulation)
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .cookie_store(true)
            .build()?;

        let storygraph = Self {
            client,
            headers: default_headers(),
            header_order: header_order(),
            cache,
        };

        let response = storygraph
            .client
            .get(HOME_URL)
            .headers(storygraph.headers.clone())
            .orig_headers(storygraph.header_order.clone())
            .send()
            .await?;

        let found = response
            .cookies()
            .any(|cookie| cookie.name() == SESSION_COOKIE);

        if !found {
            return Err(Error::MissingCookie);
        }

        Ok(storygraph)
    }

    /// Returns the cache used by this client.
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub(crate) async fn fetch(&self, path: &str, params: &[(&str, &str)]) -> Result<String> {
        let mut url = Url::parse(&format!("{APP_BASE_URL}{path}"))?;

        if !params.is_empty() {
            let mut query = url.query_pairs_mut();
            for &(key, value) in params {
                query.append_pair(key, value);
            }
        }

        let response = self
            .client
            .get(url.as_str())
            .headers(self.headers.clone())
            .orig_headers(self.header_order.clone())
            .send()
            .await?;

        let status = response.status();
        if status != StatusCode::OK {
            return Err(Error::Status {
                status: status.as_u16(),
            });
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();

        if !content_type.contains("text/html") {
            return Err(Error::ContentType(content_type));
        }

        Ok(response.text().await?)
    }
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::with_capacity(HEADERS.len());

    for &(name, value) in HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }

    headers
}

fn header_order() -> OrigHeaderMap {
    let mut order = OrigHeaderMap::with_capacity(HEADER_ORDER.len());

    for &name in HEADER_ORDER {
        order.insert(name);
    }

    order
}

pub(crate) fn split_uuids(input: &str) -> Result<Vec<Uuid>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut uuids = Vec::new();
    let mut index = 0;

    while index < input.len() {
        if index + 36 > input.len() {
            break;
        }

        let raw = input.get(index..index + 36).ok_or(Error::AuthorIds)?;
        let parsed = Uuid::parse_str(raw).map_err(|source| Error::Uuid { index, source })?;

        uuids.push(parsed);
        index += 37;
    }

    Ok(uuids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_concatenated_uuids() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let input = format!("{first},{second}");

        assert_eq!(split_uuids(&input).unwrap(), vec![first, second]);
    }

    #[test]
    fn empty_author_ids_produce_an_empty_list() {
        assert!(split_uuids("").unwrap().is_empty());
    }

    #[test]
    fn ignores_trailing_incomplete_author_ids() {
        assert!(split_uuids("not-a-uuid").unwrap().is_empty());
    }

    #[test]
    fn rejects_malformed_author_ids() {
        assert!(matches!(
            split_uuids("zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz"),
            Err(Error::Uuid { index: 0, .. })
        ));
    }
}
