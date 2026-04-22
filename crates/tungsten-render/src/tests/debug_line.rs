use super::*;

#[test]
fn debug_line_instance_layout_is_stable() {
    assert_eq!(std::mem::size_of::<DebugLineInstance>(), 40);
    assert_eq!(std::mem::align_of::<DebugLineInstance>(), 4);
}

#[test]
fn debug_line_instance_is_pod() {
    let inst = DebugLineInstance {
        a: [0.0, 0.0],
        b: [10.0, 0.0],
        thickness: 1.5,
        _pad: 0.0,
        color: [1.0, 0.0, 0.0, 1.0],
    };
    let bytes: &[u8] = bytemuck::bytes_of(&inst);
    assert_eq!(bytes.len(), std::mem::size_of::<DebugLineInstance>());
}
