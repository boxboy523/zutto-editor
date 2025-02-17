use std::env;
fn main() {
  // this method needs to be inside main() method
  env::set_var("RUST_BACKTRACE", "1");
}