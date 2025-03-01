use editor::run;
use std::{env, panic, path::PathBuf};

#[tokio::main]
async fn main() {
    log_panics::init();
    log_panics::Config::new()
        .backtrace_mode(log_panics::BacktraceMode::Off)
        .install_panic_hook();
    let args: Vec<String> = env::args().collect();
    let mut path = None;
    if args.len() > 1 {
        path = Some(PathBuf::from(&args[1]));
    }
    run(path).await.unwrap();
}