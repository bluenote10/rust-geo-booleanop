use geo_booleanop::boolean::BooleanOp;

use super::compact_geojson::write_compact_geojson;

use geo::{Coordinate, LineString, MultiPolygon, Polygon};
use geojson::{Feature, GeoJson, Geometry, Value};
use pretty_assertions::assert_eq;

use std::convert::TryInto;
use std::fs::File;
use std::io::prelude::*;
use std::panic::catch_unwind;
use std::thread::Result;

use rand::seq::SliceRandom;

pub fn load_fixture_from_path(path: &str) -> GeoJson {
    let mut file = File::open(path).expect("Cannot open/find fixture");
    let mut content = String::new();

    file.read_to_string(&mut content).expect("Unable to read fixture");

    content.parse::<GeoJson>().expect("Fixture is no geojson")
}

pub fn load_fixture(name: &str) -> GeoJson {
    load_fixture_from_path(&format!("./fixtures/{}", name))
}

pub fn fixture_polygon(name: &str) -> Polygon<f64> {
    let shape = match load_fixture(name) {
        GeoJson::Feature(feature) => feature.geometry.unwrap(),
        _ => panic!("Fixture is not a feature collection"),
    };
    shape.value.try_into().expect("Shape is not a polygon")
}

pub fn fixture_multi_polygon(name: &str) -> MultiPolygon<f64> {
    let shape = match load_fixture(name) {
        GeoJson::Feature(feature) => feature.geometry.unwrap(),
        _ => panic!("Fixture is not a feature collection"),
    };

    shape
        .value
        .clone()
        .try_into()
        .map(|p: Polygon<f64>| MultiPolygon(vec![p]))
        .or_else(|_| shape.value.try_into())
        .expect("Shape is not a multi polygon")
}

pub fn fixture_shapes(name: &str) -> (Polygon<f64>, Polygon<f64>) {
    let shapes = match load_fixture(name) {
        GeoJson::FeatureCollection(collection) => collection.features,
        _ => panic!("Fixture is not a feature collection"),
    };
    let s: Polygon<f64> = shapes[0]
        .geometry
        .as_ref()
        .unwrap()
        .value
        .clone()
        .try_into()
        .expect("Shape 1 not a polygon");
    let c: Polygon<f64> = shapes[1]
        .geometry
        .as_ref()
        .unwrap()
        .value
        .clone()
        .try_into()
        .expect("Shape 2 not a polygon");

    (s, c)
}

pub fn xy<X: Into<f64>, Y: Into<f64>>(x: X, y: Y) -> Coordinate<f64> {
    Coordinate {
        x: x.into(),
        y: y.into(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TestOperation {
    Intersection,
    Union,
    Xor,
    DifferenceAB,
    DifferenceBA,
}

#[derive(Debug)]
pub struct ExpectedResult {
    pub result: MultiPolygon<f64>,
    pub op: TestOperation,
    pub swap_ab_is_broken: bool,
}

pub fn extract_multi_polygon(feature: &Feature) -> MultiPolygon<f64> {
    let geometry_value = feature
        .geometry
        .as_ref()
        .expect("Feature must have 'geometry' property")
        .value
        .clone();
    let multi_polygon: MultiPolygon<f64> = match geometry_value {
        Value::Polygon(_) => MultiPolygon(vec![geometry_value.try_into().unwrap()]),
        Value::MultiPolygon(_) => geometry_value.try_into().unwrap(),
        _ => panic!("Feature must either be MultiPolygon or Polygon"),
    };
    multi_polygon
}

pub fn extract_expected_result(feature: &Feature) -> ExpectedResult {
    let multi_polygon = extract_multi_polygon(feature);

    let properties = feature.properties.as_ref().expect("Feature needs 'properties'.");

    let op = properties
        .get("operation")
        .expect("Feature 'properties' needs an 'operation' entry.")
        .as_str()
        .expect("'operation' entry must be a string.");

    let swap_ab_is_broken = properties
        .get("swap_ab_is_broken")
        .map(|x| x.as_bool().expect("swap_ab_is_broken must be a boolean"))
        .unwrap_or(false);

    let op = match op {
        "union" => TestOperation::Union,
        "intersection" => TestOperation::Intersection,
        "xor" => TestOperation::Xor,
        "diff" => TestOperation::DifferenceAB,
        "diff_ba" => TestOperation::DifferenceBA,
        _ => panic!(format!("Invalid operation: {}", op)),
    };

    ExpectedResult {
        result: multi_polygon,
        op,
        swap_ab_is_broken,
    }
}

pub fn update_feature(feature: &Feature, p: &MultiPolygon<f64>) -> Feature {
    let mut output_feature = feature.clone();
    output_feature.geometry = Some(Geometry::new(Value::from(p)));
    output_feature
}

pub fn load_test_case(filename: &str) -> (Vec<Feature>, MultiPolygon<f64>, MultiPolygon<f64>) {
    let original_geojson = load_fixture_from_path(filename);
    let features = match original_geojson {
        GeoJson::FeatureCollection(collection) => collection.features,
        _ => panic!("Fixture is not a feature collection"),
    };
    assert!(features.len() >= 2);
    let p1 = extract_multi_polygon(&features[0]);
    let p2 = extract_multi_polygon(&features[1]);
    (features, p1, p2)
}

pub fn apply_operation(p1: &MultiPolygon<f64>, p2: &MultiPolygon<f64>, op: TestOperation) -> MultiPolygon<f64> {
    match op {
        TestOperation::Union => p1.union(p2),
        TestOperation::Intersection => p1.intersection(p2),
        TestOperation::Xor => p1.xor(p2),
        TestOperation::DifferenceAB => p1.difference(p2),
        TestOperation::DifferenceBA => p2.difference(p1),
    }
}

#[derive(Debug)]
enum ResultTag {
    MainResult,
    SwapResult,
    PermutedResult{ab: Permutation, seed: u64}
}

#[derive(Clone, Copy, Debug)]
enum Permutation {
    AB,
    BA,
}

type WrappedResult = (ResultTag, Result<MultiPolygon<f64>>);

//use permutohedron::Heap;
//use itertools::iproduct;

/*
fn permute<T>(data: &[T]) -> Vec<Vec<T>>
where
    T: Clone,
{
    let mut data = data.to_vec();
    let heap = Heap::new(&mut data);
    heap.collect()
}
*/

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

fn randomize_line_string(line_string: &LineString<f64>, rng: &mut StdRng) -> LineString<f64> {
    if line_string.0.len() == 0 {
        line_string.clone()
    } else {
        assert!(line_string.0.first().unwrap() == line_string.0.last().unwrap());
        let old_points = line_string.0[0 .. line_string.0.len() - 1].to_vec();

        let offset = rng.gen_range(0, old_points.len() - 1);
        let reverse: bool = rng.gen();

        let mut new_points = Vec::new();
        if !reverse {
            for i in 0 .. old_points.len() {
                new_points.push(old_points[(i + offset) % old_points.len()]);
            }
        } else {
            for i in (0 .. old_points.len()).rev() {
                new_points.push(old_points[(i + offset) % old_points.len()]);
            }
        }

        if new_points.len() > 1 {
            new_points.push(new_points.first().unwrap().clone());
        }

        LineString(new_points)
    }
}

fn randomize_polygon(polygons: &MultiPolygon<f64>, seed: u64) -> MultiPolygon<f64> {
    let mut polygons = polygons.clone();

    //let mut rng = rand::thread_rng();
    let mut rng: StdRng = SeedableRng::seed_from_u64(seed);

    polygons.0.shuffle(&mut rng);
    /*
    for polygon in &mut polygons.0 {
        polygon.interiors_mut(|rings| rings.shuffle(&mut rng));
    }
    */

    let new_polygons: Vec<_> = polygons.0.iter().map(|polygon| {
        let new_exterior = randomize_line_string(&polygon.exterior(), &mut rng);
        let mut old_interiors = polygon.interiors().to_vec();
        old_interiors.shuffle(&mut rng);
        let new_interiors: Vec<_> = old_interiors.iter().map(|line_string| {
            randomize_line_string(&line_string, &mut rng)
        }).collect();
        Polygon::new(new_exterior, new_interiors)
    }).collect();

    /*
    for i in 0 .. polygons.0.len() {
        //polygons.0[i].interiors.0.shuffle(&mut rng);
        polygons.0[i].interiors_mut(|rings| rings.shuffle(&mut rng));
    }
    */

    MultiPolygon(new_polygons)
}

fn compute_all_results(
    p1: &MultiPolygon<f64>,
    p2: &MultiPolygon<f64>,
    op: TestOperation,
    skip_swap_ab: bool,
) -> Vec<WrappedResult> {
    let main_result = catch_unwind(|| {
        println!("Running operation {:?} / {:?}", op, ResultTag::MainResult);
        apply_operation(p1, p2, op)
    });

    let mut results = vec![(ResultTag::MainResult, main_result)];
    let swappable_op = match op {
        TestOperation::DifferenceAB => false,
        TestOperation::DifferenceBA => false,
        _ => true,
    };
    if swappable_op && !skip_swap_ab {
        let swap_result = catch_unwind(|| {
            println!("Running operation {:?} / {:?}", op, ResultTag::SwapResult);
            apply_operation(p2, p1, op)
        });
        results.push((ResultTag::SwapResult, swap_result));
    }

    let operand_perms = if swappable_op {
        vec![Permutation::AB, Permutation::BA]
    } else {
        vec![Permutation::AB]
    };

    for operand_perm in operand_perms {
        let (a, b) = match operand_perm {
            Permutation::AB => (p1, p2),
            Permutation::BA => (p2, p1),
        };

        for seed in 0 .. 3 {
            let a = randomize_polygon(&a, seed);
            let b = randomize_polygon(&b, seed);

            let tag = ResultTag::PermutedResult{ab: operand_perm, seed: seed};
            let result = catch_unwind(|| {
                println!("Running operation {:?} / {:?}", op, tag);
                apply_operation(&a, &b, op)
            });
            results.push((tag, result));
        }
        /*
        for (polys_a, polys_b) in iproduct!(permute(&a.0).iter(), permute(&b.0).iter()) {
            for (poly_a, poly_b) in iproduct!(polys_a, polys_b) {

            }
        }
        */
        //let poly_a_perms in a.0.iter().per

    }

    results
}

pub fn run_generic_test_case(filename: &str, regenerate: bool) -> Vec<String> {
    println!("\n *** Running test case: {}", filename);

    let (features, p1, p2) = load_test_case(filename);

    let mut output_features = vec![features[0].clone(), features[1].clone()];
    let mut failures = Vec::new();

    for feature in features.iter().skip(2) {
        let expected_result = extract_expected_result(&feature);
        let op = expected_result.op;

        let all_results = compute_all_results(&p1, &p2, op, expected_result.swap_ab_is_broken);
        for result in &all_results {
            let (result_tag, result_poly) = result;
            match &result_poly {
                Result::Err(_) => failures.push(format!("{} / {:?} / {:?} has panicked", filename, op, result_tag)),
                Result::Ok(result) => {
                    let assertion_result = std::panic::catch_unwind(|| {
                        assert_eq!(
                            *result, expected_result.result,
                            "{} / {:?} / {:?} has result deviation",
                            filename, op, result_tag,
                        )
                    });
                    if assertion_result.is_err() {
                        failures.push(format!(
                            "{} / {:?} / {:?} has result deviation",
                            filename, op, result_tag
                        ));
                    }
                }
            }
        }

        if regenerate {
            let result = all_results
                .first()
                .expect("Need at least one result")
                .1
                .as_ref()
                .expect("Regeneration mode requires a valid result");
            output_features.push(update_feature(&feature, &result));
        }
    }

    if regenerate {
        write_compact_geojson(&output_features, filename);
    }

    failures
}
