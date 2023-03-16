use std::net::SocketAddr;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app: Router = Router::new().route("/spotify", get(spotify::get_current_track_id));
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

mod spotify;
