use chrono::Utc;
use feed_rs::{model::Link, parser};

use crate::structs::{Chapter, Target};

use super::utils::make_link;

fn get_link_href(links: &[Link]) -> String {
    links.first().unwrap().href.to_owned()
}

pub fn parse_rss(target: &Target, source: &str) -> Vec<Chapter> {
    let feed = parser::parse(source.as_bytes()).unwrap();

    let mut chapters: Vec<Chapter> = vec![];
    for entry in feed.entries {
        let link = get_link_href(&entry.links);

        chapters.push(Chapter {
            manga: target.name.to_owned(),
            number: entry.id,
            title: entry.title.unwrap().content,
            date: entry.published.unwrap_or(Utc::now()),
            url: match &target.base_url {
                Some(base_url) => make_link(&base_url, &link),
                None => link,
            },
            logged_at: None,
        })
    }

    if !target.ascending_source {
        chapters.reverse();
    }

    chapters
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use chrono::DateTime;

    use crate::structs::{ParseMode, Target};

    use super::parse_rss;

    #[test]
    fn test_parse_rss() {
        let target = Target {
            name: "Test Manga".into(),
            source: "https://comic-rss.com/test.rss".into(),
            ascending_source: false,
            mode: ParseMode::Rss,
            base_url: Some("https://comic-rss.com".into()),
            request_headers: HashMap::new(),
            keys: None,
            tags: None,
        };

        let source = r###"<?xml version="1.0"?>
        <rss version="2.0" xmlns:giga="https://gigaviewer.com">
            <channel>
                <title>RSS Test Publishing</title>
                <pubDate>Fri, 23 Sep 2022 03:00:00 +0000</pubDate>
                <link>https://comic-rss.com/title/11111</link>
                <description>Lorem ipsum</description>
                <docs>http://blogs.law.harvard.edu/tech/rss</docs>
                <item>
                    <title>Part 24: The Omega</title>
                    <link>https://comic-rss.com/episode/00024</link>
                    <guid isPermalink="false">00024</guid>
                    <pubDate>Fri, 23 Sep 2022 03:00:00 +0000</pubDate>
                    <enclosure url="https://cdn-img.comic-rss.com/public/episode-thumbnail/123" length="0" type="image/jpeg" />
                    <author>Noowee</author>
                </item>
                <item>
                    <title>Part 23: The Alpha</title>
                    <link>/episode/00023</link>
                    <guid isPermalink="false">00023</guid>
                    <pubDate>Fri, 16 Sep 2022 03:00:00 +0000</pubDate>
                    <enclosure url="https://cdn-img.comic-rss.com/public/episode-thumbnail/321" length="0" type="image/jpeg" />
                    <author>Noowee</author>
                </item>
            </channel>
        </rss>
"###;
        let chapters = parse_rss(&target, source);

        // Should have 2 chapters
        assert!(chapters.len() == 2);
        // Check manga title
        assert_eq!(chapters[0].manga, "Test Manga");
        assert_eq!(chapters[0].manga, chapters[1].manga);
        // Check numbers (chapter IDs)
        assert_eq!(chapters[0].number, "00023");
        assert_eq!(chapters[1].number, "00024");
        // Check titles
        assert_eq!(chapters[0].title, "Part 23: The Alpha");
        assert_eq!(chapters[1].title, "Part 24: The Omega");
        // Check links
        assert_eq!(chapters[0].url, "https://comic-rss.com/episode/00023");
        assert_eq!(chapters[1].url, "https://comic-rss.com/episode/00024");
        // Check dates
        assert_eq!(
            chapters[0].date,
            DateTime::parse_from_rfc2822("Fri, 16 Sep 2022 03:00:00 +0000").unwrap(),
        );
        assert_eq!(
            chapters[1].date,
            DateTime::parse_from_rfc2822("Fri, 23 Sep 2022 03:00:00 +0000").unwrap(),
        );
    }
}
