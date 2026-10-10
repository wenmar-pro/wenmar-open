use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use open_server::config::Config;
use open_server::serve::Limits;
use open_server::state::AppState;

async fn shutdown() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}

async fn run() -> Result<(), String> {
    let config = Config::from_env()?;
    let port = config.port;
    let data = config.data.clone();
    let state = AppState::open(config)
        .await
        .map_err(|error| error.to_string())?;
    let meta = state.db().meta().clone();

    // `open-server check` opens the data file and stops. The image build
    // runs it, so an image with a missing or outdated data file is never made.
    if std::env::args().nth(1).as_deref() == Some("check") {
        println!(
            "data file {} is version {} ({})",
            data.display(),
            meta.data_version,
            meta.vpic_release
        );
        return Ok(());
    }

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("cannot listen on {address}: {error}"))?;
    tracing::info!(
        "serving data version {} on http://{address}",
        meta.data_version
    );
    let app = open_server::app(state);
    open_server::serve::serve(listener, app, shutdown(), Limits::default()).await;
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("open-server: {message}");
            ExitCode::FAILURE
        }
    }
}
