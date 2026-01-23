use opal_fetcher::server::run_from_env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_from_env().await?;
    Ok(())
}
