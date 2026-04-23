use sea_orm::DbErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WaydrateError {
    #[error("Something went wrong with time")]
    TimeError,
    #[error("Something went wrong with the database")]
    DatabaseError(#[from] DbErr),
}
