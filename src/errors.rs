use actix_web::http::StatusCode;
use actix_web::web::HttpResponse;
use actix_web::ResponseError;
use thiserror::Error;
use validator::ValidationErrors;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("pool error {0}")]
    PoolError(#[from] #[source] r2d2::Error),
    #[error("sql error {0}")]
    SqlError(#[from] #[source] rusqlite::Error),
    #[error("validation errors {0}")]
    ValidationErrors(#[from] #[source] ValidationErrors),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ValidationErrors(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            Self::ValidationErrors(e) => {
                HttpResponse::BadRequest().json(e.errors())
            }
            e => {
                log::error!("internal server error: {}", e);
                HttpResponse::InternalServerError().body("Internal server error.")
            }
        }
    }
}
