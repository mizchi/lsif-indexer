// Simple test file for graph query evaluation

trait Logger {
    fn log(&self, msg: &str);
}

struct ConsoleLogger;

impl Logger for ConsoleLogger {
    fn log(&self, msg: &str) {
        println!("{}", msg);
    }
}

struct FileLogger {
    path: String,
}

impl Logger for FileLogger {
    fn log(&self, msg: &str) {
        // Write to file
    }
}

fn create_logger() -> Box<dyn Logger> {
    Box::new(ConsoleLogger)
}

fn main() {
    let logger = create_logger();
    logger.log("Hello, world!");
    
    let file_logger = FileLogger {
        path: "log.txt".to_string(),
    };
    file_logger.log("Test message");
}