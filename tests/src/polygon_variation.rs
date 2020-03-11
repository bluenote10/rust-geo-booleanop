use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use geo::{LineString, MultiPolygon, Polygon};


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

pub fn randomize_polygon(polygons: &MultiPolygon<f64>, seed: u64) -> MultiPolygon<f64> {
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
