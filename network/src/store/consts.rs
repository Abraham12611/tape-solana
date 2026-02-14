pub const SECTOR_TREE_HEIGHT: usize = 10;
pub const SECTOR_LEAVES: usize = 1 << SECTOR_TREE_HEIGHT; // 1024 leaves
pub const SECTOR_BITMAP_BYTES: usize = SECTOR_LEAVES / 8;
pub const SECTOR_HEADER_BYTES: usize = SECTOR_BITMAP_BYTES + 32;

pub const SPOOL_LAYER: u8 = 1;
pub const MINER_LAYER: u8 = 2;
pub const MERKLE_ZEROS: u8 = 3;

pub const SPOOL_STORE_PRIMARY_DB: &str = "db_spoolstore";
pub const SPOOL_STORE_SECONDARY_DB_MINE: &str = "db_spoolstore_read_mine";
pub const SPOOL_STORE_SECONDARY_DB_WEB: &str = "db_spoolstore_read_web";
pub const SPOOL_STORE_SLOTS_KEY_SIZE: usize = 40; // 40 bytes
pub const SPOOL_STORE_MAX_WRITE_BUFFER_SIZE: usize = 8 * 1024 * 1024; // 8 MB
pub const SPOOL_STORE_MAX_WRITE_BUFFERS: usize = 4;
