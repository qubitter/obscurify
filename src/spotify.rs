
use reqwest::{self, Response, Url};
use reqwest::{header};

use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;

use serde::Deserialize;
use serde_json::{self, Value};

const API_BASE: &str = "https://api.spotify.com/v1/";
const ACCOUNTS_BASE: &str = "https://accounts.spotify.com/";
const API_URL: &str = "api/token";
const AUTH_URL: &str = "authorize";

#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: Option<String>,
}
#[derive(Deserialize)]
pub struct AuthReqResponse {
    pub code: String,
    pub state: String,
}

pub fn read_creds_from_file(filename: Option<&str>) -> Credentials {
    let file_content = fs::read_to_string(match filename {
        Some(string) => string,
        None => "api_keys/spotify_client.txt",
    })
    .expect("Failed to read the file containing the client credentials!");
    let out = file_content.split_once(":");
    match out {
        Some(split) => Credentials {
            username: split.0.to_owned(),
            password: Some(split.1.trim().to_owned()),
        },
        None => Credentials {
            username: file_content,
            password: None,
        },
    }
}

pub fn read_token_from_file(filename: Option<&str>) -> Credentials {
    Credentials {
        username: fs::read_to_string(match filename {
            Some(string) => string,
            None => "api_keys/spotify_token.txt",
        })
        .expect("Failed to read the file with the OAuth token."),
        password: None,
    }
}

pub fn read_client_from_file(filename: Option<&str>) -> Credentials {
    Credentials {
        username: fs::read_to_string(match filename {
            Some(string) => string,
            None => "api_keys/spotify_client.txt",
        })
        .expect("Failed to read the file with the OAuth token.")
        .split_once(":")
        .unwrap()
        .0
        .to_string(),
        password: None,
    }
}

pub async fn get_api_endpoint(accounts: bool, api_key: &str, api_endpoint: &str) -> Response {
    build_client(None)
        .expect("Failed to initialize HTTP client!")
        .get((if accounts { ACCOUNTS_BASE } else { API_BASE }).to_owned() + api_endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to send GET request!") // TODO: Make this log specific endpoint
}

/*
https://developer.spotify.com/documentation/web-api/reference/#/operations/get-the-users-currently-playing-track
body -> item -> id will yield the spotify ID for the currently playing track
*/

// https://developer.spotify.com/documentation/general/guides/authorization/client-credentials/
pub async fn get_client_credentials(creds: Credentials) -> reqwest::Response {
    let client: reqwest::Client = build_client(None).expect("Failed to initialize HTTP client");
    let mut params: HashMap<&str, &str> = HashMap::new();
    params.insert("grant_type", "client_credentials");
    client
        .post(ACCOUNTS_BASE.to_owned() + API_URL)
        .form(&params)
        .basic_auth(creds.username, creds.password)
        .send()
        .await
        .expect("Failed to send POST request!")
}

// https://developer.spotify.com/documentation/general/guides/authorization/code-flow/
pub fn get_authorization_code(
    creds: Credentials,
    scopes: Option<Vec<&str>>,
    redirect: &str,
) -> Url {
    let scps; // UGH lifetimes
    let mut params: HashMap<&str, &str> = HashMap::new();
    params.insert("client_id", creds.username.as_str());
    params.insert("response_type", "code");
    params.insert("redirect_uri", redirect);
    let _unused = match scopes {
        Some(scope) => {
            scps = scope.join(" ");
            params.insert("scope", &scps.as_str())
        }
        None => None,
    };

    let param_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}&", k, v))
        .collect::<String>();

    let mut chars = param_string.as_str().chars();
    chars.next_back();

    let param_string = chars.as_str();

    let mut url_string: String = String::new();
    url_string.push_str(ACCOUNTS_BASE);
    url_string.push_str(AUTH_URL);
    url_string.push_str("?");
    url_string.push_str(
        param_string, // a: &str, b: String; a+b = ?
    );
    Url::parse(url_string.as_str()).unwrap()
}

// https://developer.spotify.com/documentation/web-api/tutorials/code-flow
pub async fn redeem_authorization_code_for_access_token(
    authorization_code: &str,
    creds: Credentials,
    redirect: &str,
    refresh: bool,
) -> reqwest::Response {
    let mut params: HashMap<&str, &str> = HashMap::new();
    params.insert("grant_type", {
        if refresh {
            "refresh_token"
        } else {
            "authorization_code"
        }
    });
    params.insert("code", authorization_code);
    params.insert("redirect_uri", redirect);

    return build_client(None)
        .expect("Failed to initialize HTTP client")
        .post(ACCOUNTS_BASE.to_owned() + API_URL)
        .basic_auth(creds.username, creds.password)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .expect("Failed to send GET request!");
}

fn build_client(h: Option<header::HeaderMap>) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .default_headers(match h {
            Some(hd) => hd,
            None => header::HeaderMap::new(),
        })
        .build()
}
/// Retrieves an arbitrary JSON value from the given slice-encoded JSON tree.
/// Returns an Option<Value> depending on whether or not the given value was in the tree.
/// The Value will live as long as the input Value does.
pub fn retrieve_json_value<'a>(
    input: &'a Value,
    value_tree: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current_root: &Value = input;
    for child in value_tree {
        match &current_root[child] {
            Value::Null => {
                return None;
            }
            other => {
                current_root = other;
            }
        }
    }
    Some(current_root)
}
