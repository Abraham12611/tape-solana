use solana_sdk::pubkey::Pubkey;
use rocksdb::WriteBatch;
use crate::store::*;
use crate::metrics::inc_total_spools_written;

pub trait SpoolOps {
    fn put_spool_address(&self, spool_number: u64, address: &Pubkey) -> Result<(), StoreError>;
    fn get_spool_number(&self, address: &Pubkey) -> Result<u64, StoreError>;
    fn get_spool_address(&self, spool_number: u64) -> Result<Pubkey, StoreError>;
}

impl SpoolOps for SpoolStore {
    fn put_spool_address(&self, spool_number: u64, address: &Pubkey) -> Result<(), StoreError> {
        let cf_spool_by_number = self.get_cf_handle(ColumnFamily::SpoolByNumber)?;
        let cf_spool_by_address = self.get_cf_handle(ColumnFamily::SpoolByAddress)?;
        let spool_number_key = spool_number.to_be_bytes().to_vec();
        let address_key = address.to_bytes().to_vec();
        let mut batch = WriteBatch::default();
        batch.put_cf(&cf_spool_by_number, &spool_number_key, address.to_bytes());
        batch.put_cf(&cf_spool_by_address, &address_key, spool_number.to_be_bytes());
        self.db.write(batch)?;
        inc_total_spools_written();
        Ok(())
    }

    fn get_spool_number(&self, address: &Pubkey) -> Result<u64, StoreError> {
        let cf = self.get_cf_handle(ColumnFamily::SpoolByAddress)?;
        let key = address.to_bytes().to_vec();
        let spool_number_bytes = self
            .db
            .get_cf(&cf, &key)?
            .ok_or_else(|| StoreError::SpoolNotFoundForAddress(address.to_string()))?;
        Ok(u64::from_be_bytes(
            spool_number_bytes
                .try_into()
                .map_err(|_| StoreError::InvalidSegmentKey)?,
        ))
    }

    fn get_spool_address(&self, spool_number: u64) -> Result<Pubkey, StoreError> {
        let cf = self.get_cf_handle(ColumnFamily::SpoolByNumber)?;
        let key = spool_number.to_be_bytes().to_vec();
        let address_bytes = self
            .db
            .get_cf(&cf, &key)?
            .ok_or(StoreError::SpoolNotFound(spool_number))?;

        Pubkey::try_from(address_bytes.as_slice())
            .map_err(|e| StoreError::InvalidPubkey(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;
    use tempdir::TempDir;

    fn setup_store() -> Result<(SpoolStore, TempDir), StoreError> {
        let temp_dir = TempDir::new("rocksdb_test").map_err(StoreError::IoError)?;
        let store = SpoolStore::new(temp_dir.path())?;
        Ok((store, temp_dir))
    }

    #[test]
    fn test_put_spool_address() -> Result<(), StoreError> {
        let (store, _temp_dir) = setup_store()?;
        let spool_number = 1;
        let address = Pubkey::new_unique();

        store.put_spool_address(spool_number, &address)?;
        let retrieved_number = store.get_spool_number(&address)?;
        assert_eq!(retrieved_number, spool_number);
        let retrieved_address = store.get_spool_address(spool_number)?;
        assert_eq!(retrieved_address, address);
        Ok(())
    }
}
