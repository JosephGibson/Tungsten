use super::*;

#[test]
fn debug_line_instance_layout_is_stable() {
    assert_eq!(std::mem::size_of::<DebugLineInstance>(), 40);
    assert_eq!(std::mem::align_of::<DebugLineInstance>(), 4);
}
