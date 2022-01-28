use std::fs::File;
use std::fs;
use std::format;
use std::path::Path;
use std::io::Read;
use anyhow::{anyhow, Result};
use std::sync::Mutex;
use diesel::SqliteConnection;
use filecoin_proofs_api::ProverId;
use filecoin_proofs_api::seal::SealCommitPhase1Output;
use filecoin_proofs_api::SectorId;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde::{Serializer, Deserializer};

pub trait Resource {
    fn get_resource_info(&self, resource_id: String) -> Result<Vec<u8>>;
    fn store_resource_info(&self, resource: Vec<u8>) -> Result<String>;
}

pub struct FileResource {
    root: String
}


impl FileResource {
    pub fn new(root: String) -> Self {
        return FileResource{
            root
        }
    }
}
unsafe impl Send for FileResource {}
unsafe impl Sync for FileResource {}

impl Resource for FileResource {
    fn get_resource_info(&self, resource_id: String) -> Result<Vec<u8>> {
        let new_path = Path::new(self.root.as_str()).join(resource_id.clone());
        let filename = new_path.to_str().ok_or(anyhow!("unable to find file for resource {} in directory {}", resource_id.clone(), self.root.clone()))?;
        let mut f = File::open(filename)?;
        let metadata = fs::metadata(filename)?;
        let mut buffer = vec![0; metadata.len() as usize];
        f.read(&mut buffer)?;
        Ok(buffer)
    }

    fn store_resource_info(&self, resource: Vec<u8>) -> Result<String> {
        let resource_id =  Uuid::new_v4().to_string();
        let new_path = Path::new(self.root.as_str()).join(resource_id.clone());
        fs::write(new_path,resource )?;
        Ok(resource_id)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2 {
    pub prove_id: ProverId,
    pub sector_id: SectorId,
    pub phase1_output: SealCommitPhase1Output,
}