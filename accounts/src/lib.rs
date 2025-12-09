pub mod db;
pub mod models;
pub mod routes;
pub mod scheduler;

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
}
