use tokio::runtime::Runtime;

use crate::{database, error::CustomError, Factory, ResourceBuilder};
use async_trait::async_trait;

pub struct Persist;


#[async_trait]
impl ResourceBuilder<> for Persist {
    fn new() -> Self {
        Self {} 

    }

    async fn build(
        self,
        factory: &mut dyn Factory, 
        runtime: &Runtime, 
    ) -> Result<Persist, crate::Error> {


        impl Persist {
            fn save(&self){

                // Print self
                println!("{:?}", self);
                // filename is the name of the struct + .bin
                let filename = format!("{}.bin", stringify!(#name));
                let mut file = std::io::BufWriter::new(std::fs::File::create(filename).unwrap());
                bincode::serialize_into(&mut file, &self).unwrap();

            }

            fn load(&self) -> Self{
                // filename is the name of the struct + .bin
                let filename = format!("{}.bin", stringify!(#name));
                let mut file = std::fs::File::open(filename).unwrap();
                return bincode::deserialize_from(&mut file).unwrap()

            }
        }   

        Ok(Persist {})

        
    }

 
}