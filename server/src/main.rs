//! OptiCore PMS — standalone server binary.
//! The actual logic is in lib.rs so the Tauri app can embed it.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run().await
}
