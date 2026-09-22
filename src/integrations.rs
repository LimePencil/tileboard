//! Read-only external sources. Each source has its own worker, separate from OS metrics.
use crate::metrics::Source;
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub enum ExternalData {
    Git {
        branch: String,
        changed: usize,
        staged: usize,
        untracked: usize,
    },
    Service {
        status: u16,
        milliseconds: u128,
    },
    Weather {
        temperature: f64,
        feels_like: f64,
        humidity: f64,
        code: u16,
        unit: String,
    },
    Usage(UsageReport),
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsageReport {
    pub used: f64,
    pub limit: f64,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub reset_at: String,
}

pub fn setting<'a>(options: &'a toml::Table, key: &str, default: &'a str) -> &'a str {
    options
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or(default)
}

pub fn http_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).context("Enter a complete http:// or https:// URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
        "Only HTTP(S) URLs are supported"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Use an environment variable for credentials"
    );
    Ok(url)
}

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Tileboard/0.1")
        .build()?)
}

fn json<T: serde::de::DeserializeOwned>(reader: impl Read) -> Result<T> {
    let mut bytes = Vec::new();
    reader.take(1_048_577).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "Report exceeds 1 MiB");
    serde_json::from_slice(&bytes).context("Invalid report JSON")
}

pub fn collect(source: Source, options: &toml::Table, client: &Client) -> Result<ExternalData> {
    match source {
        Source::Git => git_status(setting(options, "path", "")),
        Source::Service => {
            let url = setting(options, "url", "");
            ensure!(!url.is_empty(), "Set a service URL in tile settings");
            let start = Instant::now();
            let response = client
                .head(http_url(url)?)
                .send()
                .map_err(|_| anyhow::anyhow!("Service unreachable or timed out"))?;
            Ok(ExternalData::Service {
                status: response.status().as_u16(),
                milliseconds: start.elapsed().as_millis(),
            })
        }
        Source::Weather => {
            let latitude = setting(options, "latitude", "");
            let longitude = setting(options, "longitude", "");
            ensure!(
                !latitude.is_empty() && !longitude.is_empty(),
                "Set latitude and longitude in tile settings"
            );
            let unit = setting(options, "unit", "celsius");
            let response = client
                .get("https://api.open-meteo.com/v1/forecast")
                .query(&[
                    ("latitude", latitude),
                    ("longitude", longitude),
                    (
                        "current",
                        "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code",
                    ),
                    ("temperature_unit", unit),
                ])
                .send()
                .map_err(|_| anyhow::anyhow!("Weather unavailable or timed out"))?;
            ensure!(
                response.status().is_success(),
                "Weather service returned HTTP {}",
                response.status().as_u16()
            );
            parse_weather(response, unit)
        }
        Source::Usage => {
            let source = setting(options, "source", "");
            ensure!(
                !source.is_empty(),
                "Set a usage JSON path or URL in tile settings"
            );
            let report: UsageReport =
                if source.starts_with("http://") || source.starts_with("https://") {
                    let mut request = client.get(http_url(source)?);
                    let token_env = setting(options, "token_env", "");
                    if !token_env.is_empty() {
                        let token = std::env::var(token_env)
                            .context("Usage token environment variable is not set")?;
                        request = request.bearer_auth(token);
                    }
                    let response = request
                        .send()
                        .map_err(|_| anyhow::anyhow!("Usage endpoint unavailable or timed out"))?;
                    ensure!(
                        response.status().is_success(),
                        "Usage endpoint returned HTTP {}",
                        response.status().as_u16()
                    );
                    json(response)?
                } else {
                    ensure!(
                        std::fs::metadata(source)
                            .context("Cannot read usage report")?
                            .is_file(),
                        "Usage source must be a regular file"
                    );
                    let file = File::open(source).context("Cannot open usage report")?;
                    ensure!(
                        file.metadata()?.is_file(),
                        "Usage source must be a regular file"
                    );
                    json(file)?
                };
            validate_usage(&report)?;
            Ok(ExternalData::Usage(report))
        }
        _ => bail!("Unsupported external source"),
    }
}

pub fn validate_usage(report: &UsageReport) -> Result<()> {
    ensure!(
        report.used.is_finite()
            && report.used >= 0.0
            && report.limit.is_finite()
            && report.limit > 0.0,
        "Usage requires nonnegative used and a positive limit"
    );
    Ok(())
}

pub fn parse_weather(reader: impl Read, unit: &str) -> Result<ExternalData> {
    #[derive(Deserialize)]
    struct Weather {
        current: Current,
    }
    #[derive(Deserialize)]
    struct Current {
        temperature_2m: f64,
        apparent_temperature: f64,
        relative_humidity_2m: f64,
        weather_code: u16,
    }
    let data: Weather = json(reader)?;
    let current = data.current;
    ensure!(
        current.temperature_2m.is_finite()
            && current.apparent_temperature.is_finite()
            && current.relative_humidity_2m.is_finite(),
        "Invalid weather values"
    );
    Ok(ExternalData::Weather {
        temperature: current.temperature_2m,
        feels_like: current.apparent_temperature,
        humidity: current.relative_humidity_2m,
        code: current.weather_code,
        unit: if unit == "fahrenheit" { "°F" } else { "°C" }.into(),
    })
}

pub fn git_status(path: &str) -> Result<ExternalData> {
    ensure!(!path.is_empty(), "Set a repository path in tile settings");
    // A temporary output file prevents pipe deadlocks on large repositories. No shell is involved.
    let mut output = tempfile::tempfile()?;
    let mut child = Command::new("git")
        .args([
            "--no-optional-locks",
            "-C",
            path,
            "status",
            "--porcelain=v1",
            "--branch",
            "--untracked-files=all",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(output.try_clone()?)
        .spawn()
        .context("Cannot start Git; install Git and check the repository path")?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                ensure!(status.success(), "Cannot read repository status");
                break;
            }
            Ok(None) if start.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(20))
            }
            other => {
                let _ = child.kill();
                let _ = child.wait();
                if let Err(error) = other {
                    return Err(error.into());
                }
                bail!("Git status timed out");
            }
        }
    }
    ensure!(
        output.metadata()?.len() <= 1_048_576,
        "Git status exceeds 1 MiB"
    );
    output.seek(SeekFrom::Start(0))?;
    let mut text = String::new();
    output.read_to_string(&mut text)?;
    Ok(parse_git_status(&text))
}

pub fn parse_git_status(text: &str) -> ExternalData {
    let mut branch = String::from("Unknown branch");
    let (mut changed, mut staged, mut untracked) = (0, 0, 0);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("## ") {
            branch = value.into();
        } else if line.starts_with("??") {
            untracked += 1;
        } else if line.len() >= 2 {
            changed += 1;
            if line.as_bytes()[0] != b' ' {
                staged += 1;
            }
        }
    }
    ExternalData::Git {
        branch,
        changed,
        staged,
        untracked,
    }
}
