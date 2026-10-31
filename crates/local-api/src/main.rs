use myth_local_api::{AppState, Config, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Credentials are never read or forwarded by the connected offline API.
    // Live model calls remain explicitly disabled until P6 C' is authorized.
    let config = Config::from_env()?;
    let state = AppState::new(config.mode, config.save_dir)?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    println!(
        "Myth Maker offline API: http://{} (mode: {:?})",
        listener.local_addr()?,
        config.mode
    );
    println!("Local choice recordings: {}", state.save_root().display());
    axum::serve(listener, router(state)).await?;
    Ok(())
}
