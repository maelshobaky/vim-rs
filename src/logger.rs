use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::Mutex,
};

#[derive(Debug)]
pub struct Logger {
    file: Mutex<File>,
}

impl Logger {
    pub fn new(file: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(file)
            .expect("Unable to open log file");

        Logger {
            file: Mutex::new(file),
        }
    }

    pub fn log(&self, message: &str) {
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{}", message).expect("Unable to write to log file");
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        $crate::LOGGER.get_or_init(|| $crate::Logger::new("vim-rs.log")).log(&message);
    };
}
