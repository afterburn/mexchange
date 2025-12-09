pub mod db;
pub mod mail;
pub mod models;
pub mod routes;
pub mod scheduler;

use sqlx::PgPool;

use mail::MailService;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub mail: MailService,
}
