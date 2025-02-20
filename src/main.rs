use editor::run;
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut path = None;
    if args.len() > 1 {
        path = Some(PathBuf::from(&args[1]));
    }
    run(path).await.unwrap();
}