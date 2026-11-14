//! Steg's server: the kernel plus its local socket.
//!
//! Starts on demand and outlives the app (R1-R5). This crate is the frame;
//! behaviour lands starting with TASK-1.9.
