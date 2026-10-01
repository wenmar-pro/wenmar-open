use std::process::ExitCode;

use open_server::config::Config;
use open_server::db::Db;

/// For now the binary only opens the data file and says what it holds.
/// Task 3 makes it serve.
async fn run() -> Result<(), String> {
    let config = Config::from_env()?;
    let db = Db::open(&config.data, config.connections)
        .await
        .map_err(|error| error.to_string())?;
    println!(
        "data file {} is version {} ({})",
        config.data.display(),
        db.meta().data_version,
        db.meta().vpic_release
    );
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("open-server: {message}");
            ExitCode::FAILURE
        }
    }
}
