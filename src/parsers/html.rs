use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use scraper::{ElementRef, Html, Selector};

use crate::structs::{Chapter, Target};

use super::utils::make_link;

pub(super) fn make_selector(string: &str) -> Result<Selector> {
    let selector = Selector::parse(string);
    if selector.is_err() {
        bail!("Failed creating selector: {}", string);
    }

    Ok(selector.unwrap())
}

fn get_sub_element<'a>(element: &'a ElementRef, tag: &Option<String>) -> Result<ElementRef<'a>> {
    match tag {
        Some(tag) => {
            let selector = make_selector(tag.as_str())?;
            let first_element_result = element.select(&selector).next();
            let unwrapped =
                first_element_result.ok_or(anyhow!("No element found using tag {}", tag))?;
            Ok(unwrapped)
        }
        None => Ok(element.clone()),
    }
}

pub(super) fn get_value(
    element: &ElementRef,
    tag: &Option<String>,
    attribute: &Option<String>,
) -> Result<String> {
    let element = get_sub_element(element, tag)?;
    match attribute {
        Some(attribute) => {
            let attribute_text = element.value().attr(attribute.as_str());
            let attribute_text =
                attribute_text.ok_or(anyhow!("No attribute {} found in tag", attribute))?;
            Ok(String::from(attribute_text))
        }
        None => {
            let text = element.text().collect::<String>();
            Ok(text)
        }
    }
}

fn parse_string_to_datetime(date_string: &str, format: &Option<String>) -> Result<NaiveDateTime> {
    if date_string.contains(":") {
        let datetime = match format {
            Some(format) => NaiveDateTime::parse_from_str(&date_string, format.as_str()),
            None => NaiveDateTime::from_str(&date_string),
        }?;
        Ok(datetime)
    } else {
        let date = match format {
            Some(format) => NaiveDate::parse_from_str(&date_string, format.as_str()),
            None => NaiveDate::from_str(&date_string),
        }?;
        Ok(date.and_hms_opt(0, 0, 0).unwrap())
    }
}

pub fn parse_html(target: &Target, source: &str) -> Result<Vec<Chapter>> {
    let mut chapters: Vec<Chapter> = vec![];
    let html = Html::parse_document(source);
    let tags = target.tags.as_ref().unwrap();

    let selector = make_selector(&tags.chapters_tag)?;

    for element in html.select(&selector) {
        let assemble_chapter = || -> Result<Chapter> {
            let number = get_value(&element, &tags.number_tag, &tags.number_attribute)?;

            let title = get_value(&element, &tags.title_tag, &tags.title_attribute)?;

            let get_date: Result<DateTime<Utc>> = 'setdate: {
                if tags.date_tag.is_none() && tags.date_attribute.is_none() {
                    let now = Utc::now();
                    break 'setdate Ok(now);
                }

                let date_string = get_value(&element, &tags.date_tag, &tags.date_attribute)?;
                let naive_datetime = parse_string_to_datetime(&date_string, &tags.date_format)?;

                Ok(naive_datetime.and_utc())
            };
            let date = get_date?;

            let url = get_value(&element, &tags.url_tag, &tags.url_attribute)?;

            Ok(Chapter {
                manga: target.name.to_owned(),
                number: number,
                title: title,
                date: date,
                url: match &target.base_url {
                    Some(base_url) => make_link(&base_url, &url),
                    None => url,
                },
                logged_at: None,
                announced_at: date + Duration::days(target.delay.unwrap_or(0).into()),
            })
        };

        let chapter = assemble_chapter();
        if chapter.is_err() {
            continue;
        }

        chapters.push(chapter.unwrap())
    }

    if !target.ascending_source {
        chapters.reverse();
    }

    Ok(chapters)
}

#[cfg(test)]
mod test {
    use chrono::DateTime;

    use crate::structs::{ParseMode, Target, TargetTags};

    use super::parse_html;

    #[test]
    fn test_parse_html() {
        let target = Target {
            name: "Test Manga".into(),
            source: "https://comic-html.com/test.html".into(),
            ascending_source: false,
            mode: ParseMode::Html,
            base_url: Some("https://comic-html.com".into()),
            request_headers: None,
            delay: Some(7),
            keys: None,
            tags: Some(TargetTags {
                chapters_tag: "div#chapterlist li".into(),
                number_tag: None,
                number_attribute: Some("data-num".into()),
                title_tag: Some("div div a span.chapternum".into()),
                title_attribute: None,
                date_tag: Some("div div a span.chapterdate".into()),
                date_attribute: None,
                date_format: Some("%B %-d, %Y".into()),
                url_tag: Some("div div a".into()),
                url_attribute: Some("href".into()),
            }),
        };

        let source = r###"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "https://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
        <html xmlns="https://www.w3.org/1999/xhtml" lang="ja">
        <head>
            <title>Test Manga</title>
        </head>
        <body>
            <div id="content">
                <div class="wrapper">
                    <article>
                        <div class="eplister" id="chapterlist">
                            <ul class="clstyle">
                                <li data-num="51">
                                    <div class="chbox">
                                        <div class="eph-num">
                                            <a href="https://comic-html.com/chapter/51">
                                                <span class="chapternum">Chapter 51</span>
                                                <span class="chapterdate">June 3, 2022</span>
                                            </a>
                                        </div>
                                    </div>
                                </li>
                                <li data-num="50">
                                    <div class="chbox">
                                        <div class="eph-num">
                                            <a href="https://comic-html.com/chapter/50">
                                                <span class="chapternum">Chapter 50</span>
                                                <span class="chapterdate">May 15, 2022</span>
                                            </a>
                                        </div>
                                    </div>
                                </li>
                            </ul>
                        </div>
                    </article>
                </div>
            </div>
        </body>
        </html>"###;
        let chapters = parse_html(&target, source).unwrap();

        // Should have 2 chapters
        assert!(chapters.len() == 2);
        // Check manga title
        assert_eq!(chapters[0].manga, "Test Manga");
        assert_eq!(chapters[0].manga, chapters[1].manga);
        // Check numbers (chapter IDs)
        assert_eq!(chapters[0].number, "50");
        assert_eq!(chapters[1].number, "51");
        // Check titles
        assert_eq!(chapters[0].title, "Chapter 50");
        assert_eq!(chapters[1].title, "Chapter 51");
        // Check links
        assert_eq!(chapters[0].url, "https://comic-html.com/chapter/50");
        assert_eq!(chapters[1].url, "https://comic-html.com/chapter/51");
        // Check dates
        assert_eq!(
            chapters[0].date,
            DateTime::parse_from_rfc3339("2022-05-15T00:00:00.000+00:00").unwrap(),
        );
        assert_eq!(
            chapters[1].date,
            DateTime::parse_from_rfc3339("2022-06-03T00:00:00.000+00:00").unwrap(),
        );
        // Check announce time
        assert_eq!(
            chapters[0].announced_at,
            DateTime::parse_from_rfc3339("2022-05-22T00:00:00.000+00:00").unwrap(),
        );
        assert_eq!(
            chapters[1].announced_at,
            DateTime::parse_from_rfc3339("2022-06-10T00:00:00.000+00:00").unwrap(),
        );
    }
}
