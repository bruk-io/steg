//! steg-ui (doc-17): the app's components and the design's named values. Pure interface -
//! no I/O, nothing of steg's domain (R117, R118). Organised in the four levels the design
//! itself draws as pages: foundations, atoms, molecules, organisms. Each level uses only
//! the levels before it (R119).
//!
//! Every level below is empty today (TASK-1.91): this task proves the crate's boundary, its
//! window, and how its gallery is captured, before any real component exists. TASK-1.93 adds
//! foundations' generated named values; later tasks add the first atoms, molecules and
//! organisms on top of them.

pub mod atoms;
pub mod foundations;
pub mod molecules;
pub mod organisms;
