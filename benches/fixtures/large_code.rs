// Large code sample for benchmarking
// This file contains many functions and types to test indexing performance

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

pub mod module_a {
    pub struct TypeA {
        field1: String,
        field2: i32,
        field3: Vec<u8>,
    }

    impl TypeA {
        pub fn new(name: String) -> Self {
            Self {
                field1: name,
                field2: 0,
                field3: Vec::new(),
            }
        }

        pub fn process(&mut self) -> Result<(), String> {
            self.validate()?;
            self.transform()?;
            self.output()
        }

        fn validate(&self) -> Result<(), String> {
            if self.field1.is_empty() {
                return Err("Empty name".to_string());
            }
            Ok(())
        }

        fn transform(&mut self) -> Result<(), String> {
            self.field2 += 1;
            self.field3.push(self.field2 as u8);
            Ok(())
        }

        fn output(&self) -> Result<(), String> {
            println!("Output: {} - {}", self.field1, self.field2);
            Ok(())
        }
    }

    pub fn utility_function_1(x: i32, y: i32) -> i32 {
        let result = complex_calculation(x, y);
        post_process(result)
    }

    fn complex_calculation(a: i32, b: i32) -> i32 {
        let step1 = a * b;
        let step2 = step1 + (a - b);
        let step3 = step2 % 100;
        step3
    }

    fn post_process(value: i32) -> i32 {
        value * 2
    }
}

pub mod module_b {
    use super::module_a;

    pub struct TypeB {
        data: Vec<module_a::TypeA>,
        cache: std::collections::HashMap<String, i32>,
    }

    impl TypeB {
        pub fn new() -> Self {
            Self {
                data: Vec::new(),
                cache: std::collections::HashMap::new(),
            }
        }

        pub fn add_item(&mut self, item: module_a::TypeA) {
            self.data.push(item);
        }

        pub fn process_all(&mut self) -> Vec<Result<(), String>> {
            self.data.iter_mut()
                .map(|item| item.process())
                .collect()
        }

        pub fn calculate_stats(&self) -> Stats {
            Stats {
                total: self.data.len(),
                cached: self.cache.len(),
            }
        }
    }

    pub struct Stats {
        pub total: usize,
        pub cached: usize,
    }

    impl Stats {
        pub fn summary(&self) -> String {
            format!("Total: {}, Cached: {}", self.total, self.cached)
        }
    }
}

pub mod module_c {
    use super::*;

    pub trait Processor {
        fn process(&mut self) -> Result<(), Error>;
        fn validate(&self) -> bool;
    }

    pub enum Error {
        ValidationError(String),
        ProcessingError(String),
        IOError(std::io::Error),
    }

    pub struct ComplexProcessor {
        stages: Vec<Box<dyn Processor>>,
        results: Vec<Result<(), Error>>,
    }

    impl ComplexProcessor {
        pub fn new() -> Self {
            Self {
                stages: Vec::new(),
                results: Vec::new(),
            }
        }

        pub fn add_stage(&mut self, stage: Box<dyn Processor>) {
            self.stages.push(stage);
        }

        pub fn run(&mut self) {
            for stage in &mut self.stages {
                let result = stage.process();
                self.results.push(result);
            }
        }

        pub fn get_results(&self) -> &[Result<(), Error>] {
            &self.results
        }
    }
}

pub mod module_d {
    use std::future::Future;
    use std::pin::Pin;

    pub async fn async_function_1() -> Result<String, Box<dyn std::error::Error>> {
        let result = async_helper().await?;
        Ok(format!("Result: {}", result))
    }

    async fn async_helper() -> Result<i32, Box<dyn std::error::Error>> {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(42)
    }

    pub fn create_future() -> Pin<Box<dyn Future<Output = i32> + Send>> {
        Box::pin(async { 
            let value = compute_async().await;
            value * 2
        })
    }

    async fn compute_async() -> i32 {
        100
    }
}

pub mod module_e {
    pub fn recursive_function(n: i32) -> i32 {
        if n <= 1 {
            return n;
        }
        recursive_function(n - 1) + recursive_function(n - 2)
    }

    pub fn iterative_function(n: i32) -> i32 {
        let mut a = 0;
        let mut b = 1;
        for _ in 0..n {
            let temp = a + b;
            a = b;
            b = temp;
        }
        a
    }

    pub fn complex_flow(input: Vec<i32>) -> Vec<i32> {
        input.iter()
            .filter(|&&x| x > 0)
            .map(|&x| transform_value(x))
            .filter(|&x| validate_value(x))
            .collect()
    }

    fn transform_value(x: i32) -> i32 {
        x * 2 + 1
    }

    fn validate_value(x: i32) -> bool {
        x % 3 == 0
    }
}

// Additional functions to increase complexity
pub fn function_1() { helper_1(); }
fn helper_1() { sub_helper_1(); }
fn sub_helper_1() { println!("1"); }

pub fn function_2() { helper_2(); }
fn helper_2() { sub_helper_2(); }
fn sub_helper_2() { println!("2"); }

pub fn function_3() { helper_3(); }
fn helper_3() { sub_helper_3(); }
fn sub_helper_3() { println!("3"); }

pub fn function_4() { helper_4(); }
fn helper_4() { sub_helper_4(); }
fn sub_helper_4() { println!("4"); }

pub fn function_5() { helper_5(); }
fn helper_5() { sub_helper_5(); }
fn sub_helper_5() { println!("5"); }

pub fn main_entry() {
    function_1();
    function_2();
    function_3();
    function_4();
    function_5();
    
    let result = module_a::utility_function_1(10, 20);
    println!("Result: {}", result);
    
    let mut processor = module_b::TypeB::new();
    let item = module_a::TypeA::new("test".to_string());
    processor.add_item(item);
    
    let stats = processor.calculate_stats();
    println!("{}", stats.summary());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recursive() {
        assert_eq!(module_e::recursive_function(5), 5);
    }

    #[test]
    fn test_iterative() {
        assert_eq!(module_e::iterative_function(5), 5);
    }
}