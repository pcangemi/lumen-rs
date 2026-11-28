//! Lossless GPU layouts. Geometry and shading attributes retain every original f32.
use crate::{bvh::Node, geometry::Primitive};
use anyhow::{Result, ensure};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Geometry {
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub p2: [f32; 4],
    pub info: [u32; 4],
}
impl From<&Primitive> for Geometry {
    fn from(p: &Primitive) -> Self {
        Self {
            p0: p.p0,
            p1: p.p1,
            p2: p.p2,
            info: p.info,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Attributes {
    pub n0: [f32; 4],
    pub n1: [f32; 4],
    pub n2: [f32; 4],
    pub uv01: [f32; 4],
    pub uv2: [f32; 4],
}
impl From<&Primitive> for Attributes {
    fn from(p: &Primitive) -> Self {
        Self {
            n0: p.n0,
            n1: p.n1,
            n2: p.n2,
            uv01: p.uv01,
            uv2: p.uv2,
        }
    }
}

/// Thirty index bits and two tag bits fit in the otherwise unused bound lanes.
/// Bounds are not quantized; leaf ranges and traversal order stay unchanged.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct CompactNode {
    lo: [f32; 4],
    hi: [f32; 4],
}
pub(crate) fn compact_nodes(nodes: &[Node]) -> Result<Vec<CompactNode>> {
    nodes
        .iter()
        .map(|n| {
            let leaf = n.data[1] > 0;
            let value = if leaf { n.data[1] } else { n.data[2] };
            ensure!(
                value < (1 << 30) && (leaf || n.data[3] < 3),
                "BVH index exceeds compact layout"
            );
            let tag = if leaf { 3 } else { n.data[3] };
            let mut lo = n.min;
            let mut hi = n.max;
            lo[3] = f32::from_bits(n.data[0]);
            hi[3] = f32::from_bits(value | (tag << 30));
            Ok(CompactNode { lo, hi })
        })
        .collect()
}

/// Four child boxes in structure-of-arrays form, without quantizing coordinates.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct WideNode {
    lower_x: [f32; 4],
    lower_y: [f32; 4],
    lower_z: [f32; 4],
    upper_x: [f32; 4],
    upper_y: [f32; 4],
    upper_z: [f32; 4],
    first: [u32; 4],
    count: [u32; 4],
    axes: [u32; 4],
}

pub(crate) fn wide_nodes(nodes: &[Node]) -> Result<Vec<WideNode>> {
    fn append(source: &[Node], index: usize, output: &mut Vec<WideNode>) -> Result<u32> {
        ensure!(
            output.len() < (1 << 30) - 1,
            "BVH exceeds wide child indexing"
        );
        let address = output.len() as u32;
        output.push(WideNode::zeroed());
        let root = &source[index];
        let mut node = WideNode::zeroed();
        node.count = [u32::MAX; 4];
        let mut children = [None; 4];
        if root.data[1] > 0 {
            children[0] = Some(index);
        } else {
            node.axes[0] = root.data[3];
            for (branch, child) in [root.data[0], root.data[2]].into_iter().enumerate() {
                let child_node = &source[child as usize];
                if child_node.data[1] > 0 {
                    children[branch * 2] = Some(child as usize);
                } else {
                    node.axes[branch + 1] = child_node.data[3];
                    children[branch * 2] = Some(child_node.data[0] as usize);
                    children[branch * 2 + 1] = Some(child_node.data[2] as usize);
                }
            }
        }
        for (lane, child) in children.into_iter().enumerate() {
            let Some(child) = child else { continue };
            let source_node = &source[child];
            node.lower_x[lane] = source_node.min[0];
            node.lower_y[lane] = source_node.min[1];
            node.lower_z[lane] = source_node.min[2];
            node.upper_x[lane] = source_node.max[0];
            node.upper_y[lane] = source_node.max[1];
            node.upper_z[lane] = source_node.max[2];
            node.count[lane] = source_node.data[1];
            node.first[lane] = if source_node.data[1] > 0 {
                source_node.data[0]
            } else {
                append(source, child, output)?
            };
        }
        output[address as usize] = node;
        Ok(address)
    }
    let mut output = Vec::new();
    if !nodes.is_empty() {
        append(nodes, 0, &mut output)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_bvh_retains_bounds_and_indices() {
        let nodes = [
            Node {
                min: [-1.0, -2.0, -3.0, 0.0],
                max: [1.0, 2.0, 3.0, 0.0],
                data: [1, 0, 12345678, 2],
            },
            Node {
                min: [0.0; 4],
                max: [0.0; 4],
                data: [4712345, 7, 0, 0],
            },
        ];
        let packed = compact_nodes(&nodes).unwrap();
        for (source, packed) in nodes.iter().zip(packed) {
            assert_eq!(source.min[..3], packed.lo[..3]);
            assert_eq!(source.max[..3], packed.hi[..3]);
            assert_eq!(source.data[0], packed.lo[3].to_bits());
            let tag = packed.hi[3].to_bits() >> 30;
            let value = packed.hi[3].to_bits() & 0x3fffffff;
            assert_eq!(
                source.data,
                if tag == 3 {
                    [source.data[0], value, 0, 0]
                } else {
                    [source.data[0], 0, value, tag]
                }
            );
        }
        assert_eq!(std::mem::size_of::<CompactNode>(), 32);
        assert_eq!(
            std::mem::size_of::<Geometry>() + std::mem::size_of::<Attributes>(),
            std::mem::size_of::<Primitive>()
        );
    }
    #[test]
    fn wide_bvh_preserves_all_octant_visit_orders() {
        fn binary_order(nodes: &[Node], index: usize, signs: [bool; 3], order: &mut Vec<u32>) {
            let node = &nodes[index];
            if node.data[1] > 0 {
                order.extend(node.data[0]..node.data[0] + node.data[1]);
            } else {
                let children = if signs[node.data[3] as usize] {
                    [node.data[0], node.data[2]]
                } else {
                    [node.data[2], node.data[0]]
                };
                for child in children {
                    binary_order(nodes, child as usize, signs, order);
                }
            }
        }
        fn wide_order(nodes: &[WideNode], index: usize, signs: [bool; 3], order: &mut Vec<u32>) {
            let node = &nodes[index];
            let first_branch = usize::from(!signs[node.axes[0] as usize]);
            for branch in [first_branch, first_branch ^ 1] {
                let first_child = usize::from(!signs[node.axes[branch + 1] as usize]);
                for child in [first_child, first_child ^ 1] {
                    let lane = branch * 2 + child;
                    match node.count[lane] {
                        u32::MAX => {}
                        0 => wide_order(nodes, node.first[lane] as usize, signs, order),
                        count => order.extend(node.first[lane]..node.first[lane] + count),
                    }
                }
            }
        }
        for count in [1, 7, 511] {
            let mut primitives: Vec<_> = (0..count)
                .map(|i| {
                    Primitive::sphere(
                        glam::Vec3::new((i % 11) as f32, ((i / 11) % 7) as f32, (i / 77) as f32),
                        0.1,
                        0,
                    )
                })
                .collect();
            let original = crate::bvh::Bvh::build(&mut primitives);
            let wide = wide_nodes(&original.nodes).unwrap();
            for octant in 0..8 {
                let signs = [octant & 1 == 0, octant & 2 == 0, octant & 4 == 0];
                let mut expected = Vec::new();
                let mut actual = Vec::new();
                binary_order(&original.nodes, 0, signs, &mut expected);
                wide_order(&wide, 0, signs, &mut actual);
                assert_eq!(actual, expected);
                assert_eq!(actual.len(), count);
            }
        }
    }
}
