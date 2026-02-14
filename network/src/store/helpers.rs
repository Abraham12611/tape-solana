use super::{SpoolStore, StoreError, consts::*};
use std::{env, sync::Arc};

pub fn primary() -> Result<SpoolStore, StoreError> {
    let current_dir = env::current_dir().map_err(StoreError::IoError)?;
    let db_primary = current_dir.join(SPOOL_STORE_PRIMARY_DB);
    std::fs::create_dir_all(&db_primary).map_err(StoreError::IoError)?;
    SpoolStore::new(&db_primary)
}

pub fn secondary_mine() -> Result<SpoolStore, StoreError> {
    let current_dir = env::current_dir().map_err(StoreError::IoError)?;
    let db_primary = current_dir.join(SPOOL_STORE_PRIMARY_DB);
    let db_secondary = current_dir.join(SPOOL_STORE_SECONDARY_DB_MINE);
    std::fs::create_dir_all(&db_secondary).map_err(StoreError::IoError)?;
    SpoolStore::new_secondary(&db_primary, &db_secondary)
}

pub fn secondary_web() -> Result<SpoolStore, StoreError> {
    let current_dir = env::current_dir().map_err(StoreError::IoError)?;
    let db_primary = current_dir.join(SPOOL_STORE_PRIMARY_DB);
    let db_secondary = current_dir.join(SPOOL_STORE_SECONDARY_DB_WEB);
    std::fs::create_dir_all(&db_secondary).map_err(StoreError::IoError)?;
    SpoolStore::new_secondary(&db_primary, &db_secondary)
}

pub fn read_only() -> Result<SpoolStore, StoreError> {
    let current_dir = env::current_dir().map_err(StoreError::IoError)?;
    let db_primary = current_dir.join(SPOOL_STORE_PRIMARY_DB);
    SpoolStore::new_read_only(&db_primary)
}

pub fn run_refresh_store(store: &Arc<SpoolStore>) {
    let store = Arc::clone(store);
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(15);
        loop {
            store.catch_up_with_primary().unwrap();
            tokio::time::sleep(interval).await;
        }
    });
}
