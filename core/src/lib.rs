//! Axial Memory's engine: finds Grasshopper definitions on a computer, reads
//! them without Rhino, ranks the ones linked to the palm research, and writes
//! the reports. Read-only by construction, and no networking.

pub mod classify;
pub mod cloud;
pub mod explain;
pub mod export;
pub mod gh;
pub mod map;
pub mod model;
pub mod results;
pub mod roots;
pub mod scan;
pub mod store;
pub mod terms;
