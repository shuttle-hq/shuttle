use tracing::{info, warn};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn trace_logs_are_captured() {
    // Local log
    info!("This is being logged on the info level");
    warn!("This is being logged on the warn level");

    // Ensure that certain strings are or aren't logged
    assert!(logs_contain("logged on the info level"));
    assert!(logs_contain("logged on the warn level"));
    assert!(!logs_contain("logged on the error level"));

    // Ensure that the string `logged` is logged exactly twice
    let _ = tracing_test::internal::logs_assert(|lines: &[&str]| {
        match lines.iter().filter(|line| line.contains("logged")).count() {
            2 => Ok(()),
            n => Err(format!("Expected two matching logs, but found {}", n)),
        }
    });
}
