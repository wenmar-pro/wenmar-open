use std::process::ExitCode;

use wenmar_open_cli::env::Env;

fn main() -> ExitCode {
    let env = Env::from_process();
    let code = wenmar_open_cli::run(
        std::env::args_os().collect(),
        &env,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
