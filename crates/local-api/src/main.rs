use myth_local_api::{AppState, Config, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Credential environment variables are never read, logged or passed into
    // the UI. Live mode is rejected at configuration load until P5 exists.
    let config = Config::from_env()?;
    let state = AppState::new(config.mode, config.save_dir)?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    println!("Myth Maker offline API: http://{} (mode: {:?})", listener.local_addr()?, config.mode);
    println!("Local choice recordings: {}", state.save_root().display());
    axum::serve(listener, router(state)).await?;
    Ok(())
}
