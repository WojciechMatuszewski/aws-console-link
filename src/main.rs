use std::env::{self};

use anyhow::{anyhow, Context, Ok};
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
struct Args {
    profile_name: String,

    #[clap(short, long)]
    region: String,
}

fn main() -> anyhow::Result<()> {
    let Args {
        profile_name,
        region,
    } = Args::parse();

    run(&profile_name, &region)?;

    return Ok(());
}

fn run(profile_name: &str, region: &str) -> anyhow::Result<()> {
    let env_getter: Box<EnvGetter> = Box::new(|key: &str| {
        return env::var(key).map_err(anyhow::Error::msg);
    });

    let credentials = get_aws_credentials(&profile_name, env_getter)?;
    let signin_token = get_signin_token(&credentials, &region)?;
    let console_url = get_console_url(&signin_token, &region)?;

    open::that(console_url)?;

    return Ok(());
}

fn get_console_url(signin_token: &str, region: &str) -> anyhow::Result<String> {
    let destination_url = format!(
        "https://{}.console.aws.amazon.com/console/home?region={}",
        region, region
    );

    let url = format!("https://signin.aws.amazon.com/federation");

    let url = reqwest::Url::parse_with_params(
        &url,
        &[
            ("Action", "login"),
            ("Issuer", "wojteks-app"),
            ("Destination", &destination_url),
            ("SigninToken", signin_token),
        ],
    )
    .context("Failed to build the URL")?;

    return Ok(url.into());
}

#[derive(Debug, Deserialize)]
struct GetSigninTokenResponse {
    #[serde(alias = "SigninToken")]
    signin_token: String,
}

fn get_signin_token(credentials: &Credentials, region: &str) -> anyhow::Result<String> {
    let serialized_credentials = serde_json::to_string_pretty(&credentials)
        .context("Could not serialize the credentials")?;

    let request_url = format!("https://{}.signin.aws.amazon.com/federation", region);

    let client = reqwest::blocking::Client::new();
    let res = client
        .get(request_url)
        .query(&[
            ("Action", "getSigninToken"),
            ("Session", &serialized_credentials),
        ])
        .send()
        .context("The request failed")?;

    if res.status().is_success() {
        let body = res
            .json::<GetSigninTokenResponse>()
            .context("Failed to deserialize the response")?;

        return Ok(body.signin_token);
    }

    return Err(anyhow!("Request failed"));
}

#[derive(Debug, Serialize)]
struct Credentials {
    #[serde(rename(serialize = "sessionId"))]
    access_key_id: String,
    #[serde(rename(serialize = "sessionKey"))]
    secret_access_key: String,
    #[serde(rename(serialize = "sessionToken"))]
    session_token: String,
}

type EnvGetter = dyn Fn(&str) -> anyhow::Result<String>;

fn get_aws_credentials(
    profile_name: &str,
    env_getter: Box<EnvGetter>,
) -> anyhow::Result<Credentials> {
    let exported_profile_name =
        env_getter("AWS_PROFILE").context("Missing AWS_PROFILE variable")?;

    if profile_name != exported_profile_name {
        return Err(anyhow!(
            "Request profile name different than the exported profile name"
        ));
    }

    let access_key_id =
        env_getter("AWS_ACCESS_KEY_ID").context("Missing AWS_ACCESS_KEY_ID variable")?;

    let secret_access_key =
        env_getter("AWS_SECRET_ACCESS_KEY").context("Missing AWS_SECRET_ACCESS_KEY variable")?;

    let session_token =
        env_getter("AWS_SESSION_TOKEN").context("Missing AWS_SESSION_TOKEN variable")?;

    return Ok(Credentials {
        access_key_id,
        secret_access_key,
        session_token,
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn missing_aws_profile() {
        let env_getter: Box<EnvGetter> = Box::new(|key| {
            if key == "AWS_PROFILE" {
                return Err(anyhow!("test_error"));
            }

            return Ok(String::from("foo"));
        });

        let result = get_aws_credentials("test_profile", env_getter);
        assert_eq!(true, result.is_err());

        let error_message = format!("{}", result.err().unwrap().source().unwrap());
        assert_eq!("test_error", error_message)
    }
}
