//! Steg's kernel: session, tab and pane state, and target resolution.
//!
//! The kernel is logic only. It takes data and returns data, and does no I/O
//! of its own (`.claude/rules/architecture.md`). Behaviour lands here
//! starting with TASK-1.6; this crate is the frame.
//!
//! It depends on the sdk, without the sdk's optional parts (R105), for the
//! shapes that will cross the API - unused today, because the sdk has none
//! yet.
#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    // Exercises `clippy.toml`'s `allow-unwrap-in-tests = true`: `unwrap_used`
    // is denied workspace-wide (rust.md: "No `unwrap` outside tests"), and
    // this crate has no behaviour of its own yet to test.
    #[test]
    fn a_test_may_unwrap() {
        let value: Option<i32> = "1".parse().ok();
        assert_eq!(value.unwrap(), 1);
    }
}
