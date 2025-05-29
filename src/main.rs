use anyhow::Ok;
use buffer::Buffer;
use editor::Editor;
use logger::Logger;
use once_cell::sync::OnceCell;

mod buffer;
mod editor;
mod logger;

pub static LOGGER: OnceCell<Logger> = OnceCell::new();

fn main() -> anyhow::Result<()> {
    let filepath = if std::env::args().count() > 1 {
        std::env::args().nth(1).unwrap()
    } else {
        println!(
            "You must pass a filepath!  Only recieved {} arguments.",
            std::env::args().count()
        );
        panic!()
    };

    let buffer = Buffer::from_file(&filepath)?;
    let mut editor = Editor::new(buffer)?;
    editor.run()?;
    Ok(())
}
