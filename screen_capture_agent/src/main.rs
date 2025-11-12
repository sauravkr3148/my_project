mod network;
mod qos;
use network::websocket::connect_and_stream_default;
mod codec;
#[cfg(windows)]
mod platform;

mod video_encoder;
#[tokio::main]
async fn main() {
    // Initialize logger - control with RUST_LOG environment variable
    // Examples:
    //   RUST_LOG=off          - No logging (maximum performance)
    //   RUST_LOG=error        - Only errors
    //   RUST_LOG=info         - Info and above
    //   RUST_LOG=debug        - Debug and above (verbose, like old println!)
    //   RUST_LOG=trace        - Everything
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    #[cfg(windows)]
    {
        if let Err(e) = platform::initialize_windows_features() {
            log::error!("Failed to initialize Windows features: {}", e);
        }
        platform::set_error_mode();
    }

    log::info!("Starting Video Encoding Agent...");
    log::info!("System Info:");
    #[cfg(windows)]
    {
        let (major, minor, build) = platform::get_windows_version();
        log::info!("   Windows Version: {}.{}.{}", major, minor, build);
        log::info!("   DXGI Available: {}", platform::is_dxgi_available());
        log::info!("   Elevated: {}", platform::is_elevated());
        log::info!(
            "   Optimal Capture: {:?}",
            platform::get_optimal_capture_method()
        );
    }

    if let Err(e) = connect_and_stream_default().await {
        log::error!("Application Error: {:?}", e);
    }
}
