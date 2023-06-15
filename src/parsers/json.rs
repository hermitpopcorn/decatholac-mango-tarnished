use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use json_dotpath::DotPaths;
use serde_json::Value;

use crate::structs::{
    Chapter,
    JsonDateTimeFormat::{Rfc2822, Rfc3339, UnixMilli, UnixNano, UnixSec},
    Target,
};

use super::utils::make_link;

fn parse_date_rfc2822(date_string: &str) -> Result<DateTime<Utc>> {
    let dt = DateTime::parse_from_rfc2822(date_string)?;
    Ok(dt.into())
}

fn parse_date_rfc3339(date_string: &str) -> Result<DateTime<Utc>> {
    let dt = DateTime::parse_from_rfc3339(date_string)?;
    Ok(dt.into())
}

fn parse_date_unix_seconds(timestamp: i64) -> Result<DateTime<Utc>> {
    let dt = Utc.timestamp_opt(timestamp, 0).latest().unwrap();
    Ok(dt.into())
}

fn parse_date_unix_millis(timestamp: i64) -> Result<DateTime<Utc>> {
    let dt = Utc.timestamp_millis_opt(timestamp).latest().unwrap();
    Ok(dt.into())
}

fn parse_date_unix_nanos(timestamp: i64) -> Result<DateTime<Utc>> {
    let dt = Utc.timestamp_nanos(timestamp);
    Ok(dt.into())
}

pub fn parse_json(target: &Target, source: &str) -> Result<Vec<Chapter>> {
    let mut chapters: Vec<Chapter> = vec![];
    let json: Value = serde_json::from_str(source)?;
    let keys = target.keys.as_ref().unwrap();

    let chapters_json: Value = json.dot_get(&keys.chapters)?.unwrap();
    let chapters_json = chapters_json.as_array().unwrap();

    'outer: for chapter_json in chapters_json {
        for skip_condition in &keys.skip {
            let value: Option<Value> = chapter_json.dot_get(skip_condition.0)?;
            if value.is_none() {
                continue;
            }

            let value = value.unwrap();
            if value.eq(skip_condition.1) {
                continue 'outer;
            }
        }

        let mixer = |keys: &Vec<String>| -> Result<String> {
            let mut vec = vec![];
            for key in keys {
                let the_string: Option<Value> = chapter_json.dot_get(key)?;
                let the_string = the_string.unwrap().as_str().unwrap().to_owned();
                if the_string.len() > 0 {
                    vec.push(the_string);
                }
            }
            Ok(vec.join(" "))
        };

        let number = mixer(&keys.number)?;
        let title = mixer(&keys.title)?;

        let date: Value = chapter_json.dot_get(&keys.date)?.unwrap();
        let date = match &keys.date_format {
            Some(format) => match format {
                UnixSec => parse_date_unix_seconds(date.as_i64().unwrap()),
                UnixMilli => parse_date_unix_millis(date.as_i64().unwrap()),
                UnixNano => parse_date_unix_nanos(date.as_i64().unwrap()),
                Rfc2822 => parse_date_rfc2822(date.as_str().unwrap()),
                Rfc3339 => parse_date_rfc3339(date.as_str().unwrap()),
            },
            None => parse_date_rfc3339(date.as_str().unwrap()),
        }?;

        let url: Value = chapter_json.dot_get(&keys.url)?.unwrap();
        let url = url.as_str().unwrap().to_owned();

        chapters.push(Chapter {
            manga: target.name.to_owned(),
            number: number,
            title: title,
            date: date,
            url: match &target.base_url {
                Some(base_url) => make_link(&base_url, &url),
                None => url,
            },
            logged_at: None,
        })
    }

    if !target.ascending_source {
        chapters.reverse();
    }

    Ok(chapters)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use chrono::DateTime;
    use serde_json::Value;

    use crate::structs::{ParseMode, Target, TargetKeys};

    use super::parse_json;

    #[test]
    fn test_parse_json() {
        let target = Target {
            name: "Test Manga".into(),
            source: "https://comic-json.com/test.json".into(),
            ascending_source: false,
            mode: ParseMode::Json,
            base_url: Some("https://comic-json.com".into()),
            request_headers: None,
            keys: Some(TargetKeys {
                chapters: "comic.episodes".into(),
                number: vec!["volume".into()],
                title: vec!["volume".into(), "title".into()],
                date: "publish_start".into(),
                date_format: None,
                url: "page_url".into(),
                skip: HashMap::from([(String::from("readable"), Value::Bool(false))]),
            }),
            tags: None,
        };

        let source = r###"{
            "comic": {
                "episodes": [
                    {
                        "id": 16255,
                        "volume": "Chapter 106",
                        "sort_volume": 113,
                        "page_count": 0,
                        "title": "Dat Boi",
                        "publish_start": "2022-10-11T10:00:00.000+09:00",
                        "publish_end": "2022-11-22T10:00:00.000+09:00",
                        "member_publish_start": "2022-10-11T10:00:00.000+09:00",
                        "member_publish_end": "2022-11-22T10:00:00.000+09:00",
                        "status": "public",
                        "page_url": "/comics/json/113",
                        "readable": true
                    },
                    {
                        "readable": false,
                        "title": "Sum Kinda Separator Thing"
                    },
                    {
                        "id": 16180,
                        "volume": "Chapter 105",
                        "sort_volume": 112,
                        "page_count": 0,
                        "title": "Here comes",
                        "publish_start": "2022-09-27T10:00:00.000+09:00",
                        "publish_end": "2022-11-08T10:00:00.000+09:00",
                        "member_publish_start": "2022-09-27T10:00:00.000+09:00",
                        "member_publish_end": "2022-11-08T10:00:00.000+09:00",
                        "status": "public",
                        "page_url": "/comics/json/112"
                    }
                ]
            }
        }"###;
        let chapters = parse_json(&target, source).unwrap();

        // Should have 2 chapters
        assert!(chapters.len() == 2);
        // Check manga title
        assert_eq!(chapters[0].manga, "Test Manga");
        assert_eq!(chapters[0].manga, chapters[1].manga);
        // Check numbers (chapter IDs)
        assert_eq!(chapters[0].number, "Chapter 105");
        assert_eq!(chapters[1].number, "Chapter 106");
        // Check titles
        assert_eq!(chapters[0].title, "Chapter 105 Here comes");
        assert_eq!(chapters[1].title, "Chapter 106 Dat Boi");
        // Check links
        assert_eq!(chapters[0].url, "https://comic-json.com/comics/json/112");
        assert_eq!(chapters[1].url, "https://comic-json.com/comics/json/113");
        // Check dates
        assert_eq!(
            chapters[0].date,
            DateTime::parse_from_rfc3339("2022-09-27T10:00:00.000+09:00").unwrap(),
        );
        assert_eq!(
            chapters[1].date,
            DateTime::parse_from_rfc3339("2022-10-11T10:00:00.000+09:00").unwrap(),
        );
    }
}
