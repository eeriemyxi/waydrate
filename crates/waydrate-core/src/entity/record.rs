use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "record")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    #[sea_orm(column_name = "id")]
    pub id: i32,
    #[sea_orm(column_name = "amount_ml")]
    pub amount_ml: i32,
    #[sea_orm(column_name = "date_logged")]
    pub date_logged: DateTime<Utc>,
}

impl ActiveModelBehavior for ActiveModel {}
