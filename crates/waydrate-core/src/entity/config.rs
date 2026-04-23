use sea_orm::{ActiveValue::Set, entity::prelude::*};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[sea_orm(column_name = "id")]
    pub id: i32,
    #[sea_orm(column_name = "daily_goal_ml")]
    pub daily_goal_ml: i32,
    #[sea_orm(column_name = "cup_size")]
    pub cup_size: i32,
    #[sea_orm(column_name = "display_template")]
    pub display_template: String,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            id: Set(1),
            daily_goal_ml: Set(2000),
            cup_size: Set(300),
            display_template: Set("󰖌 {cur_l}L/{max_l}L 󱌏 {cur_cup}/{max_cup}".to_owned()),
        }
    }
}
