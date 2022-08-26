use shuttle_api::{rocket, MAX_DEPLOYS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(MAX_DEPLOYS)
        .build()
        .unwrap()
        .block_on(async {
            let _rocket = rocket().await.launch().await?;

            Ok(())
        })
}
