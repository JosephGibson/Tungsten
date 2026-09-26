//! M25 ordered named-pass list. No DAG — just a `Vec<PassDesc>` executed in order.

pub mod desc;
pub mod order;
pub mod recorder;

pub use desc::{PassDesc, TargetId};
pub use order::{PassOrder, default_pass_order, text_overlay_target};
pub use recorder::PassRecorder;
