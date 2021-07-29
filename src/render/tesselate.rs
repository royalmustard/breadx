// MIT/Apache2 License

use super::{double_to_fixed, fixed_to_double, Fixed, Pointfix};
use crate::auto::render::Triangle;
use alloc::{
    vec,
    vec::{IntoIter as VecIter, Vec},
};
use core::{iter::FusedIterator, mem};
use delaunator::{triangulate, Point};
use tinyvec::ArrayVec;

/// From the given set of points, return an iterator over the triangles.
#[inline]
pub fn tesselate_shape<'a>(points: &'a [Pointfix]) -> impl Iterator<Item = Triangle> + 'a {
    let floating_points: Vec<Point> = points
        .iter()
        .copied()
        .map(|Pointfix { x, y }| Point {
            x: fixed_to_double(x),
            y: fixed_to_double(y),
        })
        .collect();

    let vector = match triangulate(&floating_points) {
        Some(t) => { std::println!("{:?}, {:?}", &t.triangles, &t.halfedges); t.triangles },
        None => vec![],
    };

    vector
        .into_iter()
        .map(move |index| &points[index])
        .copied()
        .scan(ArrayVec::<[Pointfix; 3]>::new(), |av, point| {
            av.push(point);
            if av.len() == 3 {
                let [p1, p2, p3] = mem::take(av).into_inner();
                Some(Some(Triangle { p1, p2, p3 }))
            } else {
                Some(None)
            }
        })
        .flatten()
}
