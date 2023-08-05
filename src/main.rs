use std::{collections::HashMap, io::Write};

use tokio::{fs::File, io::AsyncWriteExt};

use sqlx::{Sqlite, SqlitePool, Row, migrate::MigrateDatabase};

const DB_URL: &str = "sqlite://sqlite.db";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let info = reqwest::get("https://www.worldcubeassociation.org/api/v0/export/public")
    //     .await?
    //     .json::<serde_json::Value>()
    //     .await?;


    // let mut url = info.get("sql_url").unwrap().as_str().unwrap();

    // let mut tmp = tempfile::tempfile().unwrap();
    // println!("{}", url);

    // let zipped = reqwest::get(url).await?.bytes().await?;
    // tmp.write_all(&zipped[..]);
    // let mut zip = zip::ZipArchive::new(tmp).unwrap();
    // zip.extract("./migrations");

    Sqlite::create_database(DB_URL).await;

    let db = SqlitePool::connect(DB_URL).await.unwrap();

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let migrations = std::path::Path::new(&crate_dir).join("./migrations");
    match sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&db)
        .await {
            Ok(_) => println!("successfully ran migration"),
            Err(error) => panic!("error during migration: {}", error),
        };

    let comps = sqlx::query(
        "SELECT TABLE_NAME
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_TYPE = 'BASE TABLE' AND TABLE_CATALOG='dbName'"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    for (idx, row) in comps.iter().enumerate().take(10) {
        println!("[{}]: {:?}", idx, row.get::<String, &str>("name"));
    }

    Ok(())

}
