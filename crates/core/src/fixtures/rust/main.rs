fn main() {
    println!("Hello, world!");
    let calc = Calculator::new();
    println!("2 + 3 = {}", calc.add(2, 3));
}

struct Calculator;

impl Calculator {
    fn new() -> Self {
        Calculator
    }

    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let calc = Calculator::new();
        assert_eq!(calc.add(2, 3), 5);
    }
}