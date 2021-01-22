use actix_ratelimit::{MemoryStore, MemoryStoreActor, RateLimiter};
use actix_web::{http, middleware, web, App, HttpResponse, HttpServer};
use clap::{App as CApp, AppSettings, Arg};
use errors::AppError;
use r2d2_sqlite::{self, SqliteConnectionManager};
use serde::Deserialize;
use std::time::Duration;
use validator::Validate;

mod db;
mod errors;

type AppResponse = Result<HttpResponse, AppError>;

#[derive(Debug, Clone)]
pub struct RootUrl {
    url: String,
}

async fn get_url(pool: web::Data<db::Pool>, web::Path(id): web::Path<String>) -> AppResponse {
    let conn = pool.get()?;

    let url = db::get_url_by_id(&conn, &id).await?;

    let res;

    if let Some(url) = url {
        res = HttpResponse::Found()
            .set_header(http::header::LOCATION, url)
            .finish();
    } else {
        res = HttpResponse::NotFound().finish()
    }
    Ok(res)
}

#[derive(Debug, Deserialize, Validate)]
struct UrlPayload {
    #[validate(url)]
    url: String,
}

async fn add_url(
    pool: web::Data<db::Pool>,
    data: web::Form<UrlPayload>,
    root: web::Data<RootUrl>,
) -> AppResponse {
    data.validate()?;

    let conn = pool.get()?;

    let id;

    if let Some(u) = db::get_id_by_url(&conn, &data.url).await? {
        id = u;
    } else {
        id = db::add_url(&conn, &data.url).await?;
    }

    db::add_url_hit(&conn, &id).await?;

    Ok(HttpResponse::Created().body(format!("{}/{}", root.url, id)))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // std::env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::init();

    let matches = CApp::new("Shorust")
        .setting(AppSettings::ArgRequiredElseHelp)
        .version("1.0.0")
        .author("Edgar Luque")
        .about("Url shortener server.")
        .arg(Arg::new("root").about("The root url.").required(true))
        .arg(Arg::new("port").about("The port.").required(true))
        .arg(
            Arg::new("db_name")
                .short('d')
                .about("The database name.")
                .default_value("urls.db"),
        )
        .get_matches();

    let root_url = matches.value_of("root").unwrap();
    let port = matches.value_of("port").unwrap();
    let db_name = matches.value_of("db_name").unwrap();

    let manager = SqliteConnectionManager::file(&db_name);
    let pool = db::Pool::new(manager).unwrap();

    db::create_table(&pool.get().unwrap())
        .await
        .expect("error creating tables");

    let store = MemoryStore::new();

    let root_url = RootUrl {
        url: root_url.to_string(),
    };

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .data(root_url.clone())
            .wrap(middleware::Logger::default())
            .wrap(
                RateLimiter::new(MemoryStoreActor::from(store.clone()).start())
                    .with_interval(Duration::from_secs(60))
                    .with_max_requests(100),
            )
            .service(web::resource("/{id}").route(web::get().to(get_url)))
            .service(
                web::resource("/")
                    .route(web::post().to(add_url))
                    .route(web::get().to(|| {
                        HttpResponse::Ok()
                            .content_type("text/html")
                            .body(include_str!("index.html"))
                    })),
            )
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use std::collections::HashMap;

    #[actix_rt::test]
    async fn add_valid_url() {
        let root_url = RootUrl {
            url: "http://localhost".to_owned(),
        };

        let manager = SqliteConnectionManager::file(":memory:");
        let pool = db::Pool::new(manager).unwrap();

        db::create_table(&pool.get().unwrap())
            .await
            .expect("error creating tables");

        let mut app = test::init_service(
            App::new()
                .data(root_url)
                .data(pool.clone())
                .route("/", web::post().to(add_url)),
        )
        .await;
        let mut form = HashMap::new();
        form.insert("url", "http://twitter.com");
        let req = test::TestRequest::post().set_form(&form).to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_rt::test]
    async fn add_invalid_url() {
        let root_url = RootUrl {
            url: "http://localhost".to_owned(),
        };

        let manager = SqliteConnectionManager::file(":memory:");
        let pool = db::Pool::new(manager).unwrap();

        db::create_table(&pool.get().unwrap())
            .await
            .expect("error creating tables");

        let mut app = test::init_service(
            App::new()
                .data(root_url)
                .data(pool.clone())
                .route("/", web::post().to(add_url)),
        )
        .await;
        let mut form = HashMap::new();
        form.insert("url", "twitter.com");
        let req = test::TestRequest::post().set_form(&form).to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error());
    }


    #[actix_rt::test]
    async fn added_url_redirects() {
        let root_url = RootUrl {
            url: "http://localhost".to_owned(),
        };

        let manager = SqliteConnectionManager::file(":memory:");
        let pool = db::Pool::new(manager).unwrap();

        db::create_table(&pool.get().unwrap())
            .await
            .expect("error creating tables");

        let mut app = test::init_service(
            App::new()
                .data(root_url)
                .data(pool.clone())
                .route("/", web::post().to(add_url))
                .route("/{id}", web::get().to(get_url)),
        )
        .await;

        let mut form = HashMap::new();
        form.insert("url", "http://somedomain.com");
        let req = test::TestRequest::post().set_form(&form).to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success());

        let body = test::read_body(resp).await;
        let url_returned: &str = std::str::from_utf8(&body).unwrap();
        let id = url_returned.replace("http://localhost/", "");

        // error sadge
        let req = test::TestRequest::get().param("id", &id).to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_redirection());
    }
}
