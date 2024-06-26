mod authstate;
mod conf;
mod serve;
mod spotify;

use authstate::AuthState;
use authstate::Token;

use axum::http::HeaderMap;

use axum::http::header;
use axum::response::IntoResponse;
use axum::{extract::Query, routing::get, Router};

use axum_server::tls_rustls::RustlsConfig;
use conf::parse_args_and_render_config;
use conf::Config;
use conf::Service;

use lazy_static::lazy_static;

use reqwest::Response;
use reqwest::StatusCode as reqsc;

use parking_lot::Mutex;

use serde_json::{self, Value};
use spotify::get_api_endpoint;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::{task, time};

use rand::{distributions::Alphanumeric, Rng};

// const TRACK_URL: &str = "me/player/currently_playing";

lazy_static! {
    static ref CONFIG: Config = parse_args_and_render_config().unwrap();
    static ref REDIRECT_URI: String = CONFIG.service.redirect.clone();
}

#[tokio::main]
async fn main() {
    let svc = CONFIG.service.clone();
    let https = CONFIG.https.clone();
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
            svc.clone().domain.as_str(),
            get(move || async {
                handle_api_response(
                    CONFIG.service.clone(),
                    gae_wrapper(
                        CONFIG.service.clone().target,
                        spc,
                        CONFIG.service.clone().endpoint,
                    )
                    .await,
                )
                .await
            })
            .options(move || async {
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
    match https {
        Some(https_config) => {
            let addr = SocketAddr::from(*CONFIG.routing.get("https").unwrap());
            let config = RustlsConfig::from_pem_file(https_config.cert, https_config.key)
                .await
                .unwrap();
            match axum_server::bind_rustls(addr, config)
                .serve(app.into_make_service())
                .await
            {
                _ => (),
            }
        }
        None => {
            let addr = SocketAddr::from((
                CONFIG.routing.get("http").unwrap().0,
                CONFIG.routing.get("http").unwrap().1,
            ));
            match axum_server::bind(addr).serve(app.into_make_service()).await {
                _ => (),
            }
        }
    }
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
                REDIRECT_URI.as_str(),
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
// async fn get_current_track_id(tokens: Arc<AuthState>) -> impl IntoResponse {
//     let resp = spotify::get_api_endpoint(
//         false,
//         tokens.retrieve(Token::AccessToken).as_str(),
//         TRACK_URL,
//     )
//     .await;
// }
//
async fn gae_wrapper(target: String, aut: Arc<AuthState>, endpoint: String) -> Response {
    get_api_endpoint(
        target == "accounts",
        &aut.retrieve(Token::AccessToken),
        endpoint.as_str(),
    )
    .await
}

async fn handle_api_response(service: Service, resp: Response) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "allalike.org".parse().unwrap(),
    );
    match resp.status() {
        reqsc::NO_CONTENT => (headers, axum::http::StatusCode::NO_CONTENT).into_response(),
        reqsc::OK => {
            // FIXME: Make this logging arbitrary for request type.
            (
                headers,
                spotify::retrieve_json_value(
                    &resp
                        .json::<Value>()
                        .await
                        .expect("Failed to parse JSON in request response!"),
                    &service.extract.split("/").collect::<Vec<&str>>(),
                )
                .expect("Item ID not present in request response!")
                .to_string()
                .replace("\"", ""),
            )
                .into_response()
        }
        _ => (headers, axum::http::StatusCode::INTERNAL_SERVER_ERROR).into_response(), // don't worry about it
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
                    REDIRECT_URI.as_str(),
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
        REDIRECT_URI.as_str(),
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
    if strip_quotes(&reftok) != "null" {
        tokens.write(
            Token::RefreshToken,
            strip_quotes(&json["refresh_token"].to_string()),
        )
    }
}

fn strip_quotes(problematic_string: &String) -> String {
    let mut local_problem = problematic_string.clone();
    while local_problem.chars().nth(0) == Some('\"') || local_problem.chars().nth(0) == Some('\'') {
        local_problem.remove(0);
    }

    while local_problem.chars().last() == Some('\"') || local_problem.chars().last() == Some('\'') {
        local_problem.pop();
    }

    return local_problem;
}
