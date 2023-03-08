use reqwest::header;

use reqwest;

use std::collections::HashMap;
use std::fs;
const SPOTIFY_URL: &str = "https://api.spotify.com/me/player/currently_playing";

fn read_api_key_from_file() -> String {
    fs::read_to_string("spotify.txt").expect("Failed to read the file containing the API key!")
}

/*
https://developer.spotify.com/documentation/web-api/reference/#/operations/get-the-users-currently-playing-track
body -> item -> id will yield the spotify ID for the currently playing track
*/
pub(crate) async fn get_current_track() -> String {
    let client = build_client()
        .await
        .expect("Failed to initialize HTTP client!");
    let res = client
        .get(SPOTIFY_URL)
        .bearer_auth(read_api_key_from_file())
        .send()
        .await
        .expect("Failed to send GET request!");
    let response_string: HashMap<String, String> = match res.status() {
        reqwest::StatusCode::OK => res
            .json::<HashMap<String, String>>()
            .await
            .expect("Response was not well-formed JSON!"),
        _ => panic!("Didn't get an OK response!"),
    };
    let current_item: HashMap<String, String> = serde_json::from_str(
        response_string
            .get("item")
            .expect("Response does not have an `item` field!"),
    )
    .expect("Item field is not well-formed JSON!");
    current_item
        .get("id")
        .expect("Currently playing item does not have an ID!")
        .clone()
}

async fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    let headers = generate_headers();
    reqwest::Client::builder().default_headers(headers).build()
}

fn generate_headers() -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    headers
}
