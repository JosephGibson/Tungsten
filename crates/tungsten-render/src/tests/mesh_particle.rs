use super::*;

fn instance(x: f32) -> MeshParticleInstance {
    MeshParticleInstance {
        position: [x, 0.0],
        scale: [1.0, 1.0],
        rotation: 0.0,
        color: [255; 4],
    }
}

fn batch(mesh: u32, xs: &[f32]) -> MeshParticleBatch {
    MeshParticleBatch {
        mesh: ParticleMeshAssetId::new(mesh),
        instances: xs.iter().copied().map(instance).collect(),
    }
}

#[test]
fn mesh_particle_instance_layout_is_stable() {
    assert_eq!(std::mem::size_of::<MeshParticleInstance>(), 24);
    assert_eq!(std::mem::align_of::<MeshParticleInstance>(), 4);
    assert_eq!(std::mem::offset_of!(MeshParticleInstance, position), 0);
    assert_eq!(std::mem::offset_of!(MeshParticleInstance, scale), 8);
    assert_eq!(std::mem::offset_of!(MeshParticleInstance, rotation), 16);
    assert_eq!(std::mem::offset_of!(MeshParticleInstance, color), 20);

    let layout = MeshParticleInstance::desc();
    assert_eq!(layout.array_stride, 24);
    assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);
    let offsets: Vec<_> = layout
        .attributes
        .iter()
        .map(|attr| (attr.shader_location, attr.offset))
        .collect();
    assert_eq!(offsets, [(1, 0), (2, 8), (3, 16), (4, 20)]);
}

#[test]
fn mesh_particle_instance_is_pod() {
    let inst = MeshParticleInstance {
        position: [1.0, 2.0],
        scale: [3.0, 4.0],
        rotation: 0.5,
        color: [10, 20, 30, 40],
    };
    let bytes: &[u8] = bytemuck::bytes_of(&inst);
    assert_eq!(bytes.len(), std::mem::size_of::<MeshParticleInstance>());
    assert_eq!(&bytes[20..], &[10, 20, 30, 40]);
}

#[test]
fn plan_lays_batches_end_to_end() {
    let batches = [batch(0, &[1.0, 2.0]), batch(1, &[3.0, 4.0, 5.0])];
    let (mut staging, mut draws, mut missing) = (Vec::new(), Vec::new(), Vec::new());
    plan_draws(&batches, |_| true, &mut staging, &mut draws, &mut missing);

    assert_eq!(
        draws,
        [
            DrawRange {
                mesh: ParticleMeshAssetId::new(0),
                first: 0,
                count: 2
            },
            DrawRange {
                mesh: ParticleMeshAssetId::new(1),
                first: 2,
                count: 3
            },
        ]
    );
    let xs: Vec<f32> = staging.iter().map(|inst| inst.position[0]).collect();
    assert_eq!(xs, [1.0, 2.0, 3.0, 4.0, 5.0]);
    assert!(missing.is_empty());
}

#[test]
fn plan_skips_empty_batches_and_missing_meshes() {
    let batches = [
        batch(0, &[]),
        batch(7, &[1.0]),
        batch(1, &[2.0]),
        batch(7, &[3.0]),
    ];
    let (mut staging, mut draws, mut missing) = (Vec::new(), Vec::new(), Vec::new());
    plan_draws(
        &batches,
        |id| id.index() != 7,
        &mut staging,
        &mut draws,
        &mut missing,
    );

    assert_eq!(
        draws,
        [DrawRange {
            mesh: ParticleMeshAssetId::new(1),
            first: 0,
            count: 1
        }]
    );
    assert_eq!(staging.len(), 1);
    assert_eq!(staging[0].position[0], 2.0);
    assert_eq!(missing, [ParticleMeshAssetId::new(7)]);
}

#[test]
fn plan_clears_the_previous_frame() {
    let (mut staging, mut draws, mut missing) = (Vec::new(), Vec::new(), Vec::new());
    plan_draws(
        &[batch(0, &[1.0]), batch(3, &[2.0])],
        |id| id.index() == 0,
        &mut staging,
        &mut draws,
        &mut missing,
    );
    assert_eq!((staging.len(), draws.len(), missing.len()), (1, 1, 1));

    plan_draws(&[], |_| true, &mut staging, &mut draws, &mut missing);
    assert!(staging.is_empty());
    assert!(draws.is_empty());
    assert!(missing.is_empty());
}
