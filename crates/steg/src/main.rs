//! The server, the CLI, and the entry point that starts a server, shipped as
//! one binary (R61, doc-16). This crate holds the CLI; a thin client whose
//! commands come from the API description (R32), with no protocol code of
//! its own (R101) - and no server logic of its own (R98).
//!
//! Which of server or CLI a run acts as is not decided yet - that is
//! TASK-1.9 (the server's local socket) and TASK-1.43/TASK-1.49 (the API
//! description and the CLI's commands generated from it). This is the frame.

fn main() {
    println!("steg: not yet implemented");
}
