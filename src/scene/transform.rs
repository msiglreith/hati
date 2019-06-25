use cgmath::*;
use specs::prelude::*;

#[derive(Debug)]
pub struct LocalTransform {
    pub transform: [[f32; 4]; 4],
    pub parent: Option<Entity>,
}
impl Component for LocalTransform {
    type Storage = VecStorage<Self>;
}

impl LocalTransform {
    pub fn new(
        translation: Vector3<f32>,
        scale: f32,
        rotation: Euler<Rad<f32>>,
        parent: Option<Entity>,
    ) -> Self {
        let decomposed = Decomposed {
            scale,
            rot: Quaternion::from(rotation),
            disp: translation,
        };
        let mat: Matrix4<f32> = decomposed.into();

        LocalTransform {
            transform: mat.into(),
            parent,
        }
    }
    pub fn local_transform(&self) -> Matrix4<f32> {
        self.transform.into()
    }

    pub fn world_transform(&self, storage: &ReadStorage<LocalTransform>) -> Matrix4<f32> {
        let mut parent = self.parent;
        let mut transform = self.local_transform();
        while let Some(parent_id) = parent {
            if let Some(parent_component) = storage.get(parent_id) {
                transform = parent_component.local_transform() * transform;
                parent = parent_component.parent;
            } else {
                break;
            }
        }
        transform
    }
}
