extern crate rand;
extern crate clap;
extern crate geo_booleanop_tests;

use clap::{App, AppSettings, Arg, value_t};
use geojson::Feature;

use geo_booleanop_tests::compact_geojson::write_compact_geojson;
use geo_booleanop_tests::helper::{apply_operation, extract_expected_result, load_test_case, update_feature};
use geo_booleanop_tests::polygon_variation::randomize_polygon;

use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_generic_test_case_with_extra_options(filename: &str, swap_ab: bool, random_seed: Option<u64>) {
    println!("\n *** Running test case: {}", filename);

    let (features, p1, p2) = load_test_case(filename);

    let (p1, p2) = if !swap_ab { (p1, p2) } else { (p2, p1) };

    let (p1, p2) = match random_seed {
        Some(random_seed) => {
            (randomize_polygon(&p1, random_seed), randomize_polygon(&p2, random_seed))
        }
        _ => (p1, p2),
    };

    let mut output_features: Vec<Feature> = if !swap_ab {
        vec![features[0].clone(), features[1].clone()]
    } else {
        vec![features[1].clone(), features[0].clone()]
    };

    for feature in features.iter().skip(2) {
        let op = extract_expected_result(&feature).op;
        println!("Testing operation: {:?}", op);

        let result = apply_operation(&p1, &p2, op);

        output_features.push(update_feature(&feature, &result));
    }

    write_compact_geojson(&output_features, filename);
}

fn main() {
    #[rustfmt::skip]
    let matches = App::new("Test case runner")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(Arg::with_name("file")
                 .required(true)
                 .help("Input file"))
        .arg(Arg::with_name("swap-ab")
                 .long("swap-ab")
                 .help("Swap A/B input polygons"))
        .arg(Arg::with_name("random-permutation")
                 .long("random-permutation")
                 .short("r")
                 .takes_value(true)
                 .help("Apply random permutation to input with given seed"))
        .get_matches();

    let swap_ab = matches.is_present("swap-ab");

    let filename_in = matches.value_of("file").unwrap().to_string();
    let filename_out = filename_in.clone() + ".generated";
    fs::copy(&filename_in, &filename_out).expect("Failed to copy file.");

    let random_seed = value_t!(matches, "random-permutation", u64).ok();

    run_generic_test_case_with_extra_options(&filename_out, swap_ab, random_seed);


    // Try to run Python plot
    let script_path = Path::new(file!()).to_path_buf()
        .canonicalize().unwrap()
        .parent().unwrap().to_path_buf() // -> bin
        .parent().unwrap().to_path_buf() // -> src
        .parent().unwrap().to_path_buf() // -> tests
        .join("scripts")
        .join("plot_test_cases.py");
    Command::new(script_path.as_os_str())
        .arg("-i")
        .arg(&filename_out)
        .spawn()
        .expect("Failed to run Python plot.");
}
