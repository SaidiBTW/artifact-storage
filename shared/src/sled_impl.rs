pub async fn init_sled_db(file_path: &str) -> sled::Db {
    let _config = sled::Config::default()
        .path("/sled/my_sled_db".to_owned())
        .cache_capacity(10_000_000_000);

    let db = _config.open().unwrap();
    db
}
