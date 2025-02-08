use editor::Editor;

#[tokio::main]
async fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    let mut stdout = std::io::stdout();
    let mut editor = Editor::new(&mut stdout);
    editor.run().await.unwrap();
}