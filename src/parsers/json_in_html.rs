use anyhow::{anyhow, Result};
use scraper::Html;

use crate::structs::{Chapter, Target};

use super::{
    html::{get_value, make_selector},
    json::parse_json,
};

pub fn parse_json_in_html(target: &Target, source: &str) -> Result<Vec<Chapter>> {
    let html = Html::parse_document(source);
    let tags = target.tags.as_ref().unwrap();

    let selector = make_selector(&tags.chapters_tag)?;
    let script_tag = html
        .select(&selector)
        .next()
        .ok_or(anyhow!("Could not find script tag."))?;

    let json = get_value(&script_tag, &None, &None)?;
    parse_json(target, &json)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use chrono::DateTime;

    use crate::{
        parsers::json_in_html::parse_json_in_html,
        structs::JsonDateTimeFormat::StringFormat,
        structs::{ParseMode, Target, TargetKeys, TargetTags},
    };

    #[test]
    fn test_parse_json() {
        let target = Target {
            name: "Test Manga".into(),
            source: "https://comic-json.com/test.html".into(),
            ascending_source: false,
            mode: ParseMode::JsonInHtml,
            base_url: Some("https://comic-json.com/viewer/".into()),
            request_headers: None,
            delay: None,
            keys: Some(TargetKeys {
                chapters: "props.pageProps.chapters.0.chapters".into(),
                number: vec!["chapterId".into()],
                title: vec!["chapterMainName".into()],
                date: "updatedDate".into(),
                date_format: Some(StringFormat("%Y/%m/%d".into())),
                url: "chapterId".into(),
                skip: HashMap::new(),
            }),
            tags: Some(TargetTags {
                chapters_tag: "script#__NEXT_DATA__".into(),
                number_tag: None,
                number_attribute: None,
                title_tag: None,
                title_attribute: None,
                date_tag: None,
                date_attribute: None,
                date_format: None,
                url_tag: None,
                url_attribute: None,
            }),
        };

        let source = r###"<!DOCTYPE html>
        <html>
          <body>
            <script id="__NEXT_DATA__" type="application/json">
              {
                "props": {
                  "pageProps": {
                    "chapters": [
                      {
                        "chapters": [
                          {
                            "chapterId": 48550,
                            "chapterMainName": "CH 2",
                            "thumbnailUrl": "/e/1Bg5XY/bSC.webp?h=x3dru-Omj0uL-7U7IXKt0Q\u0026e=5000000000",
                            "pointConsumption": {},
                            "numberOfComments": 72,
                            "numberOfLikes": 1007,
                            "updatedDate": "2023/11/02",
                            "firstPageImageUrl": "/f/1Bg5Zu/bSC/0.jpeg.enc?h=9wW-UtXI8JNoAhfKMJu8HA\u0026e=1699369200"
                          },
                          {
                            "chapterId": 48286,
                            "chapterMainName": "CH 1",
                            "thumbnailUrl": "/e/1Be9Ui/bOu.webp?h=-mKnKFH-wSrp7Q8MO7tK7g\u0026e=5000000000",
                            "pointConsumption": {},
                            "numberOfComments": 17,
                            "numberOfLikes": 955,
                            "updatedDate": "2023/10/26",
                            "firstPageImageUrl": "/f/1Be9US/bOu/0.jpeg.enc?h=W5KMlod466p0jcovTgP1RQ\u0026e=1699369200"
                          }
                        ],
                        "bookIssueHeader": { "text": "ABC" }
                      }
                    ]
                  }
                }
              }
            </script>
          </body>
        </html>"###;
        let chapters = parse_json_in_html(&target, source).unwrap();

        // Should have 2 chapters
        assert!(chapters.len() == 2);
        // Check manga title
        assert_eq!(chapters[0].manga, "Test Manga");
        assert_eq!(chapters[0].manga, chapters[1].manga);
        // Check numbers (chapter IDs)
        assert_eq!(chapters[0].number, "48286");
        assert_eq!(chapters[1].number, "48550");
        // Check titles
        assert_eq!(chapters[0].title, "CH 1");
        assert_eq!(chapters[1].title, "CH 2");
        // Check links
        assert_eq!(chapters[0].url, "https://comic-json.com/viewer/48286");
        assert_eq!(chapters[1].url, "https://comic-json.com/viewer/48550");
        // Check dates
        assert_eq!(
            chapters[0].date,
            DateTime::parse_from_rfc3339("2023-10-26T00:00:00.000+00:00").unwrap(),
        );
        assert_eq!(
            chapters[1].date,
            DateTime::parse_from_rfc3339("2023-11-02T00:00:00.000+00:00").unwrap(),
        );
        // Check announce time (same as dates)
        assert_eq!(
            chapters[0].announced_at,
            DateTime::parse_from_rfc3339("2023-10-26T00:00:00.000+00:00").unwrap(),
        );
        assert_eq!(
            chapters[1].announced_at,
            DateTime::parse_from_rfc3339("2023-11-02T00:00:00.000+00:00").unwrap(),
        );
    }
}
