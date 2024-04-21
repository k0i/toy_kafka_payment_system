//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "wallets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub balance: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::payment_transactions::Entity")]
    PaymentTransactions,
}

impl Related<super::payment_transactions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PaymentTransactions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
