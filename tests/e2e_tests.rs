/// E2E test suite
// Import the E2E helper module
#[path = "e2e/mod.rs"]
mod e2e;
use e2e::*;

// Re-export all E2E test modules
#[path = "e2e/cli_basic_test.rs"]
mod cli_basic_test;

#[path = "e2e/indexing_test.rs"]
mod indexing_test;

#[path = "e2e/query_test.rs"]
mod query_test;

#[path = "e2e/differential_test.rs"]
mod differential_test;
