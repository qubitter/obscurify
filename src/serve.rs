use std::net::SocketAddr;

use axum::{extract::Path, response::Redirect, routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;

use crate::conf::{Config, HTTPSConfig};

pub async fn http_server(config: Config) -> Result<(), std::io::Error> {
    let addr = SocketAddr::from((
        config.routing.get("http").unwrap().0,
        config.routing.get("https").unwrap().1,
    ));
    let app = Router::new().route(
        "/*a",
        get(move |Path(a): Path<String>| http_upgrade(a, config.service.uri)),
    );
    axum_server::bind(addr).serve(app.into_make_service()).await
}

pub async fn http_upgrade(a: String, uri: String) -> Redirect {
    let uri = format!("https://{}{}", uri, a);
    axum::response::Redirect::temporary(uri.as_str())
}

pub async fn https_server(
    https_config: HTTPSConfig,
    app: Router<()>,
    config: Config,
) -> Result<(), std::io::Error> {
    let tls_config = match RustlsConfig::from_pem_file(https_config.cert, https_config.key).await {
        Ok(a) => a,
        Err(e) => return Err(e),
    };
    let addr = SocketAddr::from((
        config.routing.get("https").unwrap().0,
        config.routing.get("https").unwrap().1,
    ));
    match axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
    {
        _ => Ok(()), // this is a gross way of getting rust to stop yelling at us for not handling errors.
                     // to be fair, this is also a gross way of (not) handling errors.
                     // FIXME: handle errors (sigh)
    }
}
