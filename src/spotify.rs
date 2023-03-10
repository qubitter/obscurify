use reqwest::header;
use reqwest::{self, Response};

use std::collections::HashMap;
use std::fs;

use serde_json::{self, Value};

const API_BASE: &str = "https://api.spotify.com/v1/";
const ACCOUNTS_BASE: &str = "https://accounts.spotify.com/";
const TRACK_URL: &str = "me/player/currently_playing";
const AUTH_URL: &str = "api/token";

struct Credentials {
    username: String,
    password: Option<String>,
}

fn read_creds_from_file(filename: Option<&str>) -> Credentials {
    let file_content = fs::read_to_string(match filename {
        Some(string) => string,
        None => "api_keys/spotify_token.txt",
    })
    .expect("Failed to read the file containing the client credentials!");
    let out = file_content.split_once(":");
    match out {
        Some(split) => Credentials {
            username: split.0.to_owned(),
            password: Some(split.1.to_owned()),
        },
        None => Credentials {
            username: file_content,
            password: None,
        },
    }
}

fn read_token_from_file(filename: Option<&str>) -> Credentials {
    Credentials {
        username: fs::read_to_string(match filename {
            Some(string) => string,
            None => "api_keys/spotify.txt",
        })
        .expect("Failed to read the file with the OAuth token."),
        password: None,
    }
}

async fn get_api_endpoint(api_key: String, api_endpoint: &str) -> Response {
    build_client(Some(header::HeaderMap::new()))
        .expect("Failed to initialize HTTP client!")
        .get(API_BASE.to_owned() + api_endpoint)
        .bearer_auth(api_key)
        .send()
        .await
        .expect("Failed to send GET request!")
}

/*
https://developer.spotify.com/documentation/web-api/reference/#/operations/get-the-users-currently-playing-track
body -> item -> id will yield the spotify ID for the currently playing track
*/
pub(crate) async fn get_current_track_id() -> String {
    retrieve_json_value(
        &get_api_endpoint(read_token_from_file(None).username, TRACK_URL)
            .await
            .json::<Value>()
            .await
            .expect("Failed to parse JSON in track request response!"), // FIXME: Make this logging arbitrary for request type.
        &["item", "id"],
    )
    .expect("Item ID not present in track request response!")
    .to_string()
}

// https://developer.spotify.com/documentation/general/guides/authorization/client-credentials/
async fn get_client_credentials(creds: Credentials) -> reqwest::Response {
    let client: reqwest::Client = build_client(None).expect("Failed to initialize HTTP client");
    let mut params: HashMap<&str, &str> = HashMap::new();
    params.insert("grant_type", "client_credentials");
    client
        .post(ACCOUNTS_BASE.to_owned() + AUTH_URL)
        .form(&params)
        .basic_auth(creds.username, creds.password)
        .send()
        .await
        .expect("Failed to send POST request!")
}

fn build_client(h: Option<header::HeaderMap>) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .default_headers(match h {
            Some(hd) => hd,
            None => header::HeaderMap::new(),
        })
        .build()
}

fn retrieve_json_value<'a>(input: &'a Value, value_tree: &[&str]) -> Option<&'a serde_json::Value> {
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
    dbg!("{}", current_root);
    Some(current_root)
}
