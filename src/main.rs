use std::net::SocketAddr;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app: Router = Router::new().route("/spotify", get(spotify::get_current_track));
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));
    axum::Server::bind(&addr).serve(app.into_make_service());
}

mod spotify;
