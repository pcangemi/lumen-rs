//! Imported ironwork, loaded once and placed with inverse-transpose normal transforms.
use super::*;

pub(super) struct Railings {
    juliet: Vec<Primitive>,
    continuous: Vec<Primitive>,
}

impl Railings {
    pub fn load(scene: &mut Scene, material: usize) -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models/haussmann");
        let mut load = |name: &str| -> Result<Vec<Primitive>> {
            let start = scene.primitives.len();
            scene.import_obj(&root.join(name), Vec3::ZERO, 1.0, 0.0, Some(material))?;
            Ok(scene.primitives.split_off(start))
        };
        Ok(Self {
            juliet: load("juliet-balcony.obj")?,
            continuous: load("wrought-iron-balcony.obj")?,
        })
    }

    pub fn juliet(&self, scene: &mut Scene, origin: Vec3, width: f32, outward: f32) {
        self.place(
            scene,
            &self.juliet,
            origin,
            Vec3::new(width / 1.68, 1.0, 1.0),
            outward,
        );
    }

    pub fn continuous(&self, scene: &mut Scene, origin: Vec3, width: f32) {
        self.place(
            scene,
            &self.continuous,
            origin,
            Vec3::new(width / 2.36, 1.0, 1.0),
            1.0,
        );
    }

    fn place(
        &self,
        scene: &mut Scene,
        source: &[Primitive],
        origin: Vec3,
        scale: Vec3,
        outward: f32,
    ) {
        let transform = Mat3::from_cols(
            -Vec3::Z * scale.x * outward,
            Vec3::Y * scale.y,
            Vec3::X * scale.z * outward,
        );
        let normal = transform.inverse().transpose();
        scene.primitives.extend(source.iter().map(|original| {
            let mut p = *original;
            p.p0 = pack_vec3(origin + transform * unpack_vec3(p.p0), p.p0[3]);
            p.p1 = pack_vec3(transform * unpack_vec3(p.p1), 0.0);
            p.p2 = pack_vec3(transform * unpack_vec3(p.p2), 0.0);
            for n in [&mut p.n0, &mut p.n1, &mut p.n2] {
                *n = pack_vec3((normal * unpack_vec3(*n)).normalize_or_zero(), 0.0);
            }
            p
        }));
    }
}
