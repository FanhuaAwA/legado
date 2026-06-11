//! Verifies `engine_timeout_secs` actually bounds JS rule execution: a runaway
//! script is interrupted within the budget, while normal scripts still run.
//!
//! All cases live in one `#[test]` because the timeout is a process-global
//! atomic; splitting them would let parallel test threads clobber each other.

use reader_core::parser::js::{eval_js, set_js_engine_timeout_secs};
use std::time::{Duration, Instant};

#[test]
fn engine_timeout_guards_runaway_scripts() {
    // Disabled (0) is the default: a trivial script completes normally.
    set_js_engine_timeout_secs(0);
    assert!(eval_js("result = 1 + 1;", "", "").is_ok());

    // With a 1s budget, an infinite loop is interrupted promptly (not hung).
    set_js_engine_timeout_secs(1);
    let start = Instant::now();
    let result = eval_js("while (true) {}", "", "");
    let elapsed = start.elapsed();
    assert!(
        result.is_err(),
        "infinite loop should be interrupted, got {result:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(500) && elapsed < Duration::from_secs(5),
        "interrupt timing out of expected range: {elapsed:?}"
    );

    // A normal script still succeeds under a generous budget.
    set_js_engine_timeout_secs(10);
    let out = eval_js("result = input + '!';", "hi", "").expect("normal eval should succeed");
    assert_eq!(out, "hi!");

    // Restore disabled state for any further evals in this process.
    set_js_engine_timeout_secs(0);
}
