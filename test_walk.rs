use walkdir::WalkDir;

fn main() {
    let path = "tmp/sample-project";
    println!("Walking: {}", path);
    
    for entry in WalkDir::new(path) {
        match entry {
            Ok(e) => {
                println!("Found: {} (is_file: {})", e.path().display(), e.file_type().is_file());
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}