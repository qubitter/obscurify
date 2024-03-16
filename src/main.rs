mod authstate;
mod spotify;

use authstate::AuthState;
use authstate::Token;

use axum::http::HeaderMap;

use axum::response::IntoResponse;
use axum::{extract::Query, routing::get, Router};

use parking_lot::Mutex;

use reqwest::header;
use reqwest::StatusCode;

use serde_json::{self, Value};

use std::net::SocketAddr;
use std::ptr::null;
use std::sync::Arc;
use std::time::Duration;

use tokio::{task, time};

use rand::{distributions::Alphanumeric, Rng};

const TRACK_URL: &str = "me/player/currently_playing";
const REDIRECT_URI: &str = "http://127.0.0.1/authorized";

#[tokio::main]
async fn main() {
    let tokens: Arc<AuthState> = Arc::new(AuthState {
        access_token: Mutex::new(String::new()),
        refresh_token: Mutex::new(String::new()),
        token_duration: Mutex::new(String::new()),
        state_state: Mutex::new(String::new()),
    });
    let spc = tokens.clone(); // UGH
    let azd = tokens.clone(); // dumb
    let aut = tokens.clone(); // refcounts
    let app: Router = Router::new()
        .route(
            "/spotify",
            get(move || async { get_current_track_id(spc).await }).options(move || async {
                let mut headers = HeaderMap::new();
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
                headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, "*".parse().unwrap());
                headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, "*".parse().unwrap());
                headers.into_response()
            }),
        )
        .route("/authenticate", get(move || async { authorize(aut).await }))
        .route(
            "/authorized",
            get(move |query: Option<Query<Value>>| async {
                write_tokens(azd, query.unwrap()).await
            }),
        );
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));
    match axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
    {
        _ => (), // this is a gross way of getting rust to stop yelling at us for not handling errors.
                 // to be fair, this is also a gross way of (not) handling errors.
                 // FIXME: handle errors (sigh)
    };
}

/// Generates a new OAuth token if it doesn't exist.
/// Writes down the refresh token, since we'll need that eventually.
/// These are wrapped in Arc-mutexes in case two people try to load my website at the same time (unlikely!)
/// We need to pass the token through as a MutexGuard since we'll be writing to it and don't want to disrupt any ongoing reads.
async fn authorize(tokens: Arc<AuthState>) -> impl IntoResponse {
    if tokens.retrieve(Token::StateToken).eq(&String::new()) {
        tokens.write(
            Token::StateToken,
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect(),
        );
        return axum::response::Redirect::to(
            (spotify::get_authorization_code(
                spotify::read_client_from_file(None),
                Some(vec!["user-read-currently-playing"]),
                REDIRECT_URI,
            )
            .as_str()
            .to_owned()
                + format!("&state={}", tokens.retrieve(Token::StateToken)).as_str())
            .as_str(),
        )
        .into_response();
    } else {
        return "Access token already present! No need for further authorization. Rock on :)"
            .into_response();
    }
}

// fn bullshit(reqr: reqwest::Response) -> impl IntoResponse {
//     let (parts, body) = reqr.res.into_parts();
//     let body = Body::stream(body);
//     http::Response::from_parts(parts, body)
// }

/// Entry point into the Spotify crate to get the current track.
/// Has you pass in the finished OAuth token for authorization.
/// Basically just a wrapper around spotify::get_api_endpoint and spotify::retrieve_json_value.
/// The token value is only being read, so it's fine to pass it through as a string.
async fn get_current_track_id(tokens: Arc<AuthState>) -> impl IntoResponse {
    let resp = spotify::get_api_endpoint(
        false,
        tokens.retrieve(Token::AccessToken).as_str(),
        TRACK_URL,
    )
    .await;

    match resp.status() {
        StatusCode::NO_CONTENT => StatusCode::NO_CONTENT.into_response(),
        StatusCode::OK => {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
            // FIXME: Make this logging arbitrary for request type.
            (
                headers,
                spotify::retrieve_json_value(
                    &resp
                        .json::<Value>()
                        .await
                        .expect("Failed to parse JSON in track request response!"),
                    &["item", "id"],
                )
                .expect("Item ID not present in track request response!")
                .to_string(),
            )
                .into_response()
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(), // don't worry about it
    }
}

/// Serves as our final step in the Spotify authorization flow.
/// Writes down the OAuth token and the refresh token we get from authorize().
/// Takes both in as MutexGuards so that we can write them down.
async fn write_tokens(tokens: Arc<AuthState>, query: Query<Value>) -> String {
    match query.get("state") {
        None => panic!("Should have gotten a state back from the auth code request!"),
        Some(s) => {
            if tokens.retrieve(Token::StateToken) != s.as_str().unwrap() {
                panic!(
                    "Received an incorrect state of {} when expecting {}!",
                    s,
                    tokens.retrieve(Token::StateToken)
                )
            } else {
                let code: &str = match query.get("code") {
                    Some(s) => s.as_str().unwrap(),
                    None => panic!("{}", query["error"].as_str().unwrap()),
                };

                let response = spotify::redeem_authorization_code_for_access_token(
                    code,
                    spotify::read_creds_from_file(None),
                    REDIRECT_URI,
                    false,
                )
                .await;

                let response_json = response
                    .json::<Value>()
                    .await
                    .expect("Failed to parse JSON of authorization code redemption response!");

                let mut access_token = response_json["access_token"].to_string();
                access_token.pop();
                access_token.remove(0);

                tokens.write(Token::AccessToken, access_token);

                let mut refresh_token = response_json["refresh_token"].to_string();
                refresh_token.pop();
                refresh_token.remove(0);

                tokens.write(Token::RefreshToken, refresh_token);

                tokens.write(
                    Token::TokenDuration,
                    response_json["expires_in"].to_string(),
                );

                task::spawn(async move {
                    loop {
                        time::sleep(Duration::from_secs(
                            response_json["expires_in"].as_u64().unwrap() - 300,
                        ))
                        .await;
                        refresh_tokens(tokens.clone()).await
                    }
                });
                return "Successfully authorized! You can close this page now.".to_owned();
            }
        }
    }
}

async fn refresh_tokens(tokens: Arc<AuthState>) -> () {
    let response = spotify::redeem_authorization_code_for_access_token(
        tokens.retrieve(Token::RefreshToken).as_str(),
        spotify::read_creds_from_file(None),
        REDIRECT_URI,
        true,
    )
    .await;

    let json = response.json::<Value>().await.unwrap();

    tokens.write(
        Token::AccessToken,
        strip_quotes(&json["access_token"].to_string()),
    );
    tokens.write(
        Token::TokenDuration,
        strip_quotes(&json["expires_in"].to_string()),
    );
    let reftok = json["refresh_token"].to_string();
    if (strip_quotes(&reftok) != "null") {
        tokens.write(
            Token::RefreshToken,
            strip_quotes(&json["refresh_token"].to_string()),
        )
    }
}

fn strip_quotes(problematic_string: &String) -> String {
    let mut local_problem = problematic_string.clone();
    while (local_problem.chars().nth(0) == Some('\"') || local_problem.chars().nth(0) == Some('\''))
    {
        local_problem.remove(0);
    }

    while (local_problem.chars().last() == Some('\"') || local_problem.chars().last() == Some('\''))
    {
        local_problem.pop();
    }

    return local_problem;
}
