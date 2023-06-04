use url::Url;

/// Helper that appends the target's base URL if the URL is relative
pub fn make_link(base_url: &str, link: &str) -> String {
    let url = match Url::parse(link) {
        Ok(url) => url,
        Err(_) => {
            let merged = String::from(base_url) + link;
            Url::parse(&merged).unwrap()
        }
    };
    url.into()
}
