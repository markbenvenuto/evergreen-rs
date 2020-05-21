// Copyright [2020] [Mark Benvenuto]
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use reqwest::header;
use reqwest::Url;
use std::fmt::Write;
use std::fs::File;
use std::str::FromStr;
use std::string::String;
use structopt::StructOpt;

use json::JsonValue;
use regex::Regex;

use log::info;

use evergreen_rs_derive::EvgFields;

#[macro_use]
extern crate anyhow;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct EvergreenConfig {
    // A full URL
    // http://evergreen-api.mongodb.com:8080/api
    api_server_host: String,

    // A full URL
    // https://evergreen.mongodb.com
    ui_server_host: String,
    api_key: String,
    user: String,
}

fn get_hosts_url(config: &EvergreenConfig, user: &str) -> Url {
    Url::parse(&format!(
        "{}/rest/v2/users/{}/hosts",
        config.api_server_host, user
    ))
    .unwrap()
}

#[derive(Debug, PartialEq, Serialize, Deserialize, EvgFields)]
struct Distro {
    distro_id: String,
    provider: String,
    image_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, EvgFields)]
struct Tag {
    key: String,
    value: String,
    can_be_modified: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, EvgFields)]
struct Host {
    host_id: String,
    host_url: String,
    distro: Distro,
    provisioned: bool,
    started_by: String,
    host_type: String,
    user: String,
    status: String,
    // running_task: {
    //   task_id: null,
    //   name: null,
    //   dispatch_time: null,
    //   version_id: null,
    //   build_id: null
    // },
    user_host: bool,
    no_expiration: bool,
    instance_tags: Vec<Tag>,
    instance_type: String,
    zone: String,
    display_name: String,
    home_volume_id: String,
}

struct EvergreenClient {
    config: EvergreenConfig,

    client: reqwest::blocking::Client,
}

impl EvergreenClient {
    fn new_from_home() -> Result<EvergreenClient> {
        let home_dir_opt = dirs::home_dir();
        if home_dir_opt.is_none() {
            eprintln!("Must set an home directory");
            return Err(anyhow!("Could not find the user home directory"));
        }
        let evg_file = home_dir_opt.unwrap().to_str().unwrap().to_owned();
        let filename = evg_file + "/.evergreen.yml";
        let file = File::open(filename)?;

        let config: EvergreenConfig = serde_yaml::from_reader(file)?;
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Api-User",
            header::HeaderValue::from_str(&config.user).expect("Bad Api-User"),
        );
        headers.insert(
            "Api-Key",
            header::HeaderValue::from_str(&config.api_key).expect("Bad Api-Key"),
        );

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(EvergreenClient {
            config: config,
            client: client,
        })
    }

    fn get_hosts(&self, user: Option<&str>) -> Result<Vec<Host>> {
        let url = get_hosts_url(&self.config, user.unwrap_or(&self.config.user));
        let resp = self.client.get(url).send()?.text()?;

        let v: Vec<Host> = serde_json::from_str(&resp)?;
        Ok(v)
    }
}

#[derive(Debug)]
enum OutputType {
    Flat,
    Json,
}

impl FromStr for OutputType {
    type Err = anyhow::Error;
    fn from_str(day: &str) -> Result<Self, Self::Err> {
        match day {
            "json" => Ok(OutputType::Json),
            "flat" => Ok(OutputType::Flat),
            _ => Err(anyhow!("Could not parse a on output type")),
        }
    }
}

#[derive(StructOpt, Debug)]
/// Dumps spawn hosts from evergreen
struct Cli {
    #[structopt(
        short = "o",
        long = "output",
        default_value = "flat",
        case_insensitive = true
    )]
    output: OutputType,

    // Display only the URL
    #[structopt(long)]
    url: bool,

    // List of entries for hosts to display matching a regex
    #[structopt(short, long)]
    filter: Option<String>,
}

fn to_flat_json_int(v: &JsonValue, prefix: &str, writer: &mut dyn Write) -> Result<()> {
    match v {
        JsonValue::Null => {
            write!(writer, "{}:null\n", prefix)?;
        }
        JsonValue::Short(s) => {
            write!(writer, "{}:{}\n", prefix, s)?;
        }
        JsonValue::String(s) => {
            write!(writer, "{}:{}\n", prefix, s)?;
        }
        JsonValue::Number(n) => {
            write!(writer, "{}:{}\n", prefix, n)?;
        }
        JsonValue::Boolean(b) => {
            write!(writer, "{}:{}\n", prefix, b)?;
        }
        JsonValue::Object(o) => {
            for field in o.iter() {
                if prefix == "" {
                    to_flat_json_int(&field.1, field.0, writer)?;
                } else {
                    to_flat_json_int(&field.1, &format!("{}.{}", prefix, field.0), writer)?;
                }
            }
        }
        JsonValue::Array(arr) => {
            for (i, member) in arr.iter().enumerate() {
                if prefix == "" {
                    to_flat_json_int(member, &format!("{}", i), writer)?;
                } else {
                    to_flat_json_int(member, &format!("{}.{}", prefix, i), writer)?;
                }
            }
        }
    }

    Ok(())
}

fn to_flat_json(s: &str) -> Result<String> {
    let v = json::parse(s)?;

    let mut r = String::new();
    to_flat_json_int(&v, "", &mut r)?;
    Ok(r)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::from_args();

    info!("args : {:?}", args);

    let client = EvergreenClient::new_from_home()?;

    let hosts = client.get_hosts(Option::None)?;

    let mut filter: Option<Regex> = Option::None;
    if let Some(filt) = args.filter {
        filter = Some(Regex::new(&filt)?);
    }

    for host in hosts {
        let flat = to_flat_json(&serde_json::to_string_pretty(&host)?)?;

        if let Some(filt) = filter.as_ref() {
            if !filt.is_match(&flat) {
                continue;
            }
        }

        match args.url {
            true => {
                println!("{}@{}", host.user, host.host_url);
            }
            false => match args.output {
                OutputType::Flat => {
                    println!("{}", flat);
                }
                OutputType::Json => {
                    println!("{}", serde_json::to_string_pretty(&host)?);
                }
            },
        }
    }

    Ok(())
}

#[test]
fn test_flat_json_array() {
    assert_eq! { to_flat_json(r#"["a","b"]"#).unwrap(),
r#"0:a
1:b
"#};
}

#[test]
fn test_flat_json_obj() {
    assert_eq! { to_flat_json(r#"{"a":"b", "n":42}"#).unwrap(),
r#"a:b
n:42
"#};
}

#[test]
fn test_flat_json_obj_nested() {
    assert_eq! { to_flat_json(r#"{"a": { "n":42 } }"#).unwrap(),
r#"a.n:42
"#};
}

#[test]
fn test_flat_json_array_obj_nested() {
    assert_eq! { to_flat_json(r#"[{"a": { "n":42 } }]"#).unwrap(),
r#"0.a.n:42
"#};
}
