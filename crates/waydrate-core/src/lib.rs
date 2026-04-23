pub mod entity;
pub mod error;
use chrono::{Local, Utc};
use entity::{config, record};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    sea_query::OnConflict,
};

pub async fn establish_connection(url: &str) -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect(url).await?;

    db.get_schema_registry("waydrate_core::entity::*")
        .sync(&db)
        .await?;

    config::Entity::insert(config::ActiveModel::new())
        .on_conflict(
            OnConflict::column(config::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&db)
        .await?;

    Ok(db)
}

pub async fn set_goal(conn: &DatabaseConnection, ml: i32) -> Result<(), error::WaydrateError> {
    let existing_config = config::Entity::find_by_id(1)
        .one(conn)
        .await?
        .expect("config row id 1 should always exist");
    let mut active_config: config::ActiveModel = existing_config.into();
    active_config.daily_goal_ml = Set(ml);
    active_config.update(conn).await?;
    Ok(())
}

pub async fn set_cup_size(conn: &DatabaseConnection, ml: i32) -> Result<(), error::WaydrateError> {
    let existing_config = config::Entity::find_by_id(1)
        .one(conn)
        .await?
        .expect("config row id 1 should always exist");
    let mut active_config: config::ActiveModel = existing_config.into();
    active_config.cup_size = Set(ml);
    active_config.update(conn).await?;
    Ok(())
}

pub async fn set_display_template(
    conn: &DatabaseConnection,
    template: String,
) -> Result<(), error::WaydrateError> {
    let existing_config = config::Entity::find_by_id(1)
        .one(conn)
        .await?
        .expect("config row id 1 should always exist");
    let mut active_config: config::ActiveModel = existing_config.into();
    active_config.display_template = Set(template);
    active_config.update(conn).await?;
    Ok(())
}

pub async fn get_config(
    conn: &DatabaseConnection,
) -> Result<Option<config::Model>, error::WaydrateError> {
    Ok(config::Entity::find_by_id(1).one(conn).await?)
}

pub async fn add_record(conn: &DatabaseConnection, ml: i32) -> Result<(), error::WaydrateError> {
    let new_rec = record::ActiveModel {
        id: NotSet,
        amount_ml: Set(ml),
        date_logged: Set(Utc::now()),
    };
    record::Entity::insert(new_rec).exec(conn).await?;
    Ok(())
}

pub async fn remove_record(conn: &DatabaseConnection, id: i32) -> Result<(), error::WaydrateError> {
    record::Entity::delete_by_id(id).exec(conn).await?;
    Ok(())
}

pub async fn get_daily_records(
    conn: &DatabaseConnection,
) -> Result<Vec<record::Model>, error::WaydrateError> {
    let today_local = Local::now().date_naive();

    let day_start_local = today_local
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| error::WaydrateError::TimeError)?
        .and_local_timezone(Local)
        .latest()
        .ok_or_else(|| error::WaydrateError::TimeError)?;

    let day_end_local = today_local
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| error::WaydrateError::TimeError)?
        .and_local_timezone(Local)
        .latest()
        .ok_or_else(|| error::WaydrateError::TimeError)?;

    let start_utc = day_start_local.with_timezone(&Utc);
    let end_utc = day_end_local.with_timezone(&Utc);

    let records = record::Entity::find()
        .filter(record::Column::DateLogged.between(start_utc, end_utc))
        .order_by_asc(record::Column::DateLogged)
        .all(conn)
        .await?;

    Ok(records)
}

pub async fn get_daily_total(
    conn: &sea_orm::DatabaseConnection,
) -> Result<i32, error::WaydrateError> {
    let records = get_daily_records(conn).await?;
    let total_ml = records.iter().map(|r| r.amount_ml).sum();
    Ok(total_ml)
}

pub async fn close_connection(db: DatabaseConnection) -> Result<(), DbErr> {
    db.close().await
}

#[cfg(test)]
mod tests {}
