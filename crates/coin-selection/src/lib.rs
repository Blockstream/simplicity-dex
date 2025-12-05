pub mod common;
pub mod sqlite_db;
pub mod types;

#[cfg(test)]
mod tests {
    use sqlx::{Row, Sqlite, SqlitePool, migrate::MigrateDatabase};
    const DB_URL: &str = "sqlite://sqlite.db";

    #[tokio::test]
    async fn it_works() {
        if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
            println!("Creating database {}", DB_URL);
            match Sqlite::create_database(DB_URL).await {
                Ok(_) => println!("Create db success"),
                Err(error) => panic!("error: {}", error),
            }
        } else {
            println!("Database already exists");
        }
        let db = SqlitePool::connect(DB_URL).await.unwrap();
        let result = sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY NOT NULL, name VARCHAR(250) NOT NULL);",
        )
        .execute(&db)
        .await
        .unwrap();
        println!("Create user table result: {:?}", result);
        let result = sqlx::query(
            "SELECT name
         FROM sqlite_schema
         WHERE type ='table'
         AND name NOT LIKE 'sqlite_%';",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        for (idx, row) in result.iter().enumerate() {
            println!("[{}]: {:?}", idx, row.get::<String, &str>("name"));
        }
    }
}

// todo: create interface with functions
// add_maker_fund_tx
// add_taker_fund_tx
// get_utxos_maker_fund
// Tokens has to be returned -> Result<Utxos{
// utxo1: Option<String>
// }>
