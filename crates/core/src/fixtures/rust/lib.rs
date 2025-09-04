pub mod utils {
    pub fn format_name(name: &str) -> String {
        format!("Hello, {}!", name)
    }
}

pub trait Greeter {
    fn greet(&self) -> String;
}

pub struct Person {
    name: String,
}

impl Person {
    pub fn new(name: String) -> Self {
        Person { name }
    }
}

impl Greeter for Person {
    fn greet(&self) -> String {
        format!("Hi, I'm {}", self.name)
    }
}