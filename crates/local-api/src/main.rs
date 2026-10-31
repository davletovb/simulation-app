use myth_local_api::{AppState, Config, router};
use tower_http::services::ServeDir;

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
    let assets = std::env::var("MYTH_WEB_DIST").unwrap_or_else(|_| "apps/web/dist".into());
    // Never serve the private campaign/save directory, even if configured as
    // a static asset root. A separate trust boundary is required.
    let assets_path = std::path::Path::new(&assets);
    if assets_path.exists() && assets_path.canonicalize()?.starts_with(state.save_root()) {
        return Err("private saved data cannot be a web asset root".into());
    }
    let site = router(state)
        .fallback_service(ServeDir::new(assets).append_index_html_on_directories(true));
    axum::serve(listener, site).await?;
    Ok(())
}
