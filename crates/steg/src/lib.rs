//! The CLI, and the entry point that starts a server, shipped as one binary
//! with the server (doc-16, R61, R98). This library crate holds what
//! `src/main.rs` and this crate's own tests share; `main.rs` keeps the one
//! hand-written case (`server start`/`server start --on-demand`, TASK-1.77's
//! own dependency note) and no server logic of its own (R98).

pub mod cli;
