mod lib;

use lib::{Database, create_database};

fn main() {
    let db = create_database();
    
    match db.connect() {
        Ok(conn) => {
            println!("Connected!");
            if let Err(e) = conn.execute("SELECT * FROM users") {
                eprintln!("Query failed: {:?}", e);
            }
        }
        Err(e) => {
            eprintln!("Connection failed: {:?}", e);
        }
    }
}