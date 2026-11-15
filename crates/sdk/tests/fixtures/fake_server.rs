//! A tiny stand-in for "a server now accepts there"
//! (`local_socket::connect`'s own tests, `crates/sdk/src/local_socket.rs`).
//! Its whole job is one call into the library crate's own test-support
//! function, which does what a real server is contracted to do for this
//! purpose: resolve the socket path from its inherited environment, the
//! same way `connect`'s spawn step expects, and bind there. Its own argv
//! is ignored - a real `steg server start --on-demand` is not told a path
//! on its argv either.

fn main() {
    steg_sdk::run_fake_server_for_tests();
}
