// Test sample for call hierarchy analysis

fn main() {
    let result = calculate(10, 20);
    println!("Result: {}", result);
    
    let processor = DataProcessor::new();
    processor.process();
}

fn calculate(a: i32, b: i32) -> i32 {
    let sum = add(a, b);
    let product = multiply(a, b);
    combine(sum, product)
}

fn add(x: i32, y: i32) -> i32 {
    x + y
}

fn multiply(x: i32, y: i32) -> i32 {
    x * y
}

fn combine(sum: i32, product: i32) -> i32 {
    sum + product
}

struct DataProcessor {
    data: Vec<i32>,
}

impl DataProcessor {
    fn new() -> Self {
        Self {
            data: vec![1, 2, 3, 4, 5],
        }
    }
    
    fn process(&self) {
        self.validate();
        let result = self.transform();
        self.output(result);
    }
    
    fn validate(&self) {
        println!("Validating data...");
        for item in &self.data {
            self.check_item(*item);
        }
    }
    
    fn check_item(&self, item: i32) {
        if item < 0 {
            panic!("Invalid item: {}", item);
        }
    }
    
    fn transform(&self) -> Vec<i32> {
        self.data.iter()
            .map(|x| self.transform_item(*x))
            .collect()
    }
    
    fn transform_item(&self, item: i32) -> i32 {
        item * 2
    }
    
    fn output(&self, result: Vec<i32>) {
        println!("Output: {:?}", result);
    }
}

// Call hierarchy:
// main
//   ├── calculate
//   │   ├── add
//   │   ├── multiply
//   │   └── combine
//   └── DataProcessor::process
//       ├── DataProcessor::validate
//       │   └── DataProcessor::check_item
//       ├── DataProcessor::transform
//       │   └── DataProcessor::transform_item
//       └── DataProcessor::output