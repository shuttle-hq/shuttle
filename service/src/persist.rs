use tokio::runtime::Runtime;

use crate::{Factory, ResourceBuilder};
use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;

use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;

use bincode;
use bincode::serialize_into;

use std::error::Error;

pub struct Persist;

impl Persist {
    pub fn save<T: Serialize>(&self, struc: T){
        let file = File::create("data.bin").unwrap();
        let mut writer = BufWriter::new(file);
        serialize_into(&mut writer, &struc).unwrap();
    }

    pub fn load<T>(&self) -> Result<T, Box<dyn Error>>
    where
        T: DeserializeOwned,
    {
        let file = File::open("data.bin").unwrap();
        let reader = BufReader::new(file);
        let data = bincode::deserialize_from(reader)?;
        Ok(data)
    }

}   


#[async_trait]
impl ResourceBuilder<Persist> for Persist {
    fn new() -> Self {
        Self {} 
    }

    async fn build(
        self,
        _factory: &mut dyn Factory, 
        _runtime: &Runtime, 
    ) -> Result<Persist, crate::Error> {

        Ok(Persist {})

        
    }


}

// Create Test
#[cfg(test)]
mod tests {}