use std::{collections::HashMap, env, fs};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value as JsonValue};
use toml::{map::Map, Value as TomlValue};

use crate::structs::{JsonDateTimeFormat, ParseMode, Target, TargetKeys, TargetTags};

pub fn get_config(filename: Option<&str>) -> Result<TomlValue> {
    let filename = filename.unwrap_or("settings.toml");

    let mut exe_path = env::current_exe()?;
    exe_path.pop();
    exe_path.push(filename);

    let mut cwd_path = env::current_dir()?;
    cwd_path.push(filename);

    let tries = vec![exe_path, cwd_path];

    let mut file_contents = None;
    for path in tries {
        let file = fs::read(path);
        if file.is_ok() {
            file_contents = Some(file?);
            break;
        }
    }

    if file_contents.is_none() {
        bail!("Missing config file.")
    }

    let config: String = String::from_utf8_lossy(&file_contents.unwrap()).into_owned();
    let config: toml::Value = config.parse()?;

    Ok(config)
}

pub fn get_targets(config: Option<&TomlValue>) -> Result<Vec<Target>> {
    if config.is_none() {
        bail!("No targets found.")
    }

    let config_targets = config
        .unwrap()
        .as_array()
        .ok_or(anyhow!("Target config is not an array."))?;

    let mut targets = vec![];
    for config_target in config_targets {
        targets.push(Target {
            name: convert_value_to_string(config_target, "name")?,
            source: convert_value_to_string(config_target, "source")?,
            ascending_source: config_target
                .get("ascending_source")
                .unwrap_or(&TomlValue::Boolean(false))
                .as_bool()
                .unwrap_or(false),
            mode: match config_target
                .get("mode")
                .ok_or(anyhow!("No mode in target."))?
                .as_str()
                .unwrap()
            {
                "rss" => ParseMode::Rss,
                "json" => ParseMode::Json,
                "html" => ParseMode::Html,
                other => bail!("Invalid mode in target: {}", other),
            },
            base_url: match config_target.get("baseUrl") {
                Some(value) => Some(value.as_str().unwrap().to_owned()),
                None => None,
            },
            request_headers: match config_target.get("requestHeaders") {
                Some(table) => Some(convert_toml_map_to_string_hashmap(
                    table.as_table().unwrap(),
                )),
                None => None,
            },
            keys: parse_keys(config_target.get("keys"))?,
            tags: parse_tags(config_target.get("tags"))?,
        })
    }

    Ok(targets)
}

fn parse_keys(toml_keys: Option<&TomlValue>) -> Result<Option<TargetKeys>> {
    if toml_keys.is_none() {
        return Ok(None);
    }

    let config_keys = toml_keys.unwrap();

    Ok(Some(TargetKeys {
        chapters: convert_value_to_string(config_keys, "chapters")?,
        number: convert_value_to_array_of_string(config_keys, "number")?,
        title: convert_value_to_array_of_string(config_keys, "title")?,
        date: convert_value_to_string(config_keys, "date")?,
        date_format: match convert_value_to_string(config_keys, "dateFormat") {
            Ok(the_string) => match the_string.as_str() {
                "unixsec" => Some(JsonDateTimeFormat::UnixSec),
                "unix" | "unixmilli" => Some(JsonDateTimeFormat::UnixMilli),
                "unixnano" => Some(JsonDateTimeFormat::UnixNano),
                "rfc2822" => Some(JsonDateTimeFormat::Rfc2822),
                "rfc3339" => Some(JsonDateTimeFormat::Rfc3339),
                _ => None,
            },
            Err(_) => None,
        },
        url: convert_value_to_string(config_keys, "url")?,
        skip: convert_table_to_hashmap(config_keys, "skip")?,
    }))
}

fn parse_tags(toml_tags: Option<&TomlValue>) -> Result<Option<TargetTags>> {
    if toml_tags.is_none() {
        return Ok(None);
    }

    let config_tags = toml_tags.unwrap();

    let disallow_empty = |converted_string: Result<String>| -> Option<String> {
        if converted_string.is_err() {
            return None;
        }

        let converted_string = converted_string.unwrap();
        if converted_string.len() < 1 {
            return None;
        }

        Some(converted_string)
    };

    Ok(Some(TargetTags {
        chapters_tag: convert_value_to_string(config_tags, "chaptersTag")?,
        number_tag: disallow_empty(convert_value_to_string(config_tags, "numberTag")),
        number_attribute: disallow_empty(convert_value_to_string(config_tags, "numberAttribute")),
        title_tag: disallow_empty(convert_value_to_string(config_tags, "titleTag")),
        title_attribute: disallow_empty(convert_value_to_string(config_tags, "titleAttribute")),
        date_tag: disallow_empty(convert_value_to_string(config_tags, "dateTag")),
        date_attribute: disallow_empty(convert_value_to_string(config_tags, "dateAttribute")),
        date_format: disallow_empty(convert_value_to_string(config_tags, "date_format")),
        url_tag: disallow_empty(convert_value_to_string(config_tags, "urlTag")),
        url_attribute: disallow_empty(convert_value_to_string(config_tags, "urlAttribute")),
    }))
}

fn convert_value_to_string(value: &TomlValue, name: &str) -> Result<String> {
    let result = value
        .get(name)
        .ok_or(anyhow!("No {} in target keys.", name))?
        .as_str()
        .unwrap()
        .to_owned();

    Ok(result)
}

fn convert_value_to_array_of_string(value: &TomlValue, name: &str) -> Result<Vec<String>> {
    let value = value
        .get(name)
        .ok_or(anyhow!("No {} in target keys.", name))?;

    let mut vector = vec![];
    if value.is_array() {
        let values = value.as_array().unwrap();
        for v in values {
            let converted_string = v.as_str().unwrap().to_owned();
            if converted_string.len() > 0 {
                vector.push(converted_string);
            }
        }
    } else {
        let converted_string = value.as_str().unwrap().to_owned();
        if converted_string.len() > 0 {
            vector.push(converted_string);
        }
    }

    Ok(vector)
}

fn convert_toml_map_to_string_hashmap(
    toml_map: &Map<String, TomlValue>,
) -> HashMap<String, String> {
    let mut hashmap = HashMap::new();

    for kv in toml_map {
        hashmap.insert(kv.0.to_owned(), kv.1.to_string());
    }

    hashmap
}

fn convert_toml_map_to_value_hashmap(
    toml_map: &Map<String, TomlValue>,
) -> HashMap<String, JsonValue> {
    let mut hashmap = HashMap::new();

    for kv in toml_map {
        hashmap.insert(kv.0.to_owned(), json!(kv.1));
    }

    hashmap
}

fn convert_table_to_hashmap(value: &TomlValue, name: &str) -> Result<HashMap<String, JsonValue>> {
    let result = match value.get(name) {
        Some(table) => convert_toml_map_to_value_hashmap(table.as_table().unwrap()),
        None => HashMap::new(),
    };

    Ok(result)
}
