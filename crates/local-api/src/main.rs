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
    if assets_path.exists() && asset_root_overlaps_private(assets_path, state.save_root())? {
        return Err("private saved data cannot be a web asset root".into());
    }
    let site = router(state)
        .fallback_service(ServeDir::new(assets).append_index_html_on_directories(true));
    axum::serve(listener, site).await?;
    Ok(())
}

fn asset_root_overlaps_private(
    assets: &std::path::Path,
    saves: &std::path::Path,
) -> std::io::Result<bool> {
    // Reject both containment directions, including symlinked or relative
    // paths: ServeDir must never traverse into the private Hiyal journal.
    let assets = assets.canonicalize()?;
    let saves = saves.canonicalize()?;
    Ok(assets.starts_with(&saves) || saves.starts_with(&assets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_both_static_save_root_overlap_directions() {
        let root = std::env::temp_dir().join(format!("p6-asset-root-{}", std::process::id()));
        let web = root.join("web");
        let saves = web.join("local-data/saves");
        std::fs::create_dir_all(&saves).unwrap();
        assert!(asset_root_overlaps_private(&web, &saves).unwrap());
        assert!(asset_root_overlaps_private(&saves, &web).unwrap());
        let other = root.join("outside");
        std::fs::create_dir(&other).unwrap();
        assert!(!asset_root_overlaps_private(&other, &saves).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
