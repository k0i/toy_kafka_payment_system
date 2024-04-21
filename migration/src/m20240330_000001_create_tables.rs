use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS `wallets` (
                `id` int NOT NULL AUTO_INCREMENT PRIMARY KEY,
                `balance` int NOT NULL DEFAULT 0
            )",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS `payment_transactions` (
                `id` int NOT NULL AUTO_INCREMENT PRIMARY KEY,
                `wallet_id` int NOT NULL,
                `amount` int NOT NULL,
                FOREIGN KEY (`wallet_id`) REFERENCES `wallets` (`id`)
            )",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE `wallets`")
            .await?;

        Ok(())
    }
}
