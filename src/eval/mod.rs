use log::{debug, info};

use crate::{
    derivation::Derivation,
    messages::{BuildRequest, BuildResponseV1, BuildStatus},
};

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufRead,
    path::PathBuf,
    process::{Command, Output},
};

fn log_command_output(output: Output) {
    for line in output.stderr.lines() {
        info!("stderr: {:?}", line)
    }

    for line in output.stdout.lines() {
        debug!("stdout: {:?}", line)
    }
}

pub fn load_r13y_log(rev: &str) -> Vec<BuildResponseV1> {
    if let Ok(log_file) = File::open(format!("reproducibility-log-{}.json", rev)) {
        serde_json::from_reader(log_file).expect("Unable to parse r13y log")
    } else {
        Vec::new()
    }
}

pub struct JobInstantiation {
    pub results: Vec<BuildResponseV1>,
    pub to_build: HashSet<PathBuf>,
    pub skip_list: HashSet<PathBuf>,
}

pub fn eval(instruction: BuildRequest) -> JobInstantiation {
    let job = match instruction {
        BuildRequest::V1(ref req) => req.clone(),
    };

    let mut results = Vec::new();

    let mut skip_list = HashSet::new();
    let prev_results = load_r13y_log(&job.nar_hash);
    for elem in prev_results.into_iter() {
        if elem.status == BuildStatus::FirstFailed {
            info!(
                "Ignoring for skiplist as it failed the first time: {:#?}",
                &elem
            );
        } else {
            skip_list.insert(PathBuf::from(&elem.drv));
            results.push(elem);
        }
    }

    let attr_name = job.attr.join(".");

    info!("Evaluating {}#{}", job.flake_url, attr_name);
    let eval = Command::new("nix")
        .arg("path-info")
        .arg("--derivation")
        .arg("--recursive")
        .arg("--impure")
        .arg(format!("{}#{}", &job.flake_url, &attr_name))
        .output()
        .expect("failed to execute process");

    let to_build = eval
        .stdout
        .lines()
        .filter_map(|line_result| {
            line_result
                .ok()
                .and_then(|line| line.ends_with(".drv").then(||line.into()))
        })
        .collect();

    log_command_output(eval);

    info!("Evaluating {}#{}", job.flake_url, attr_name);
    let show = Command::new("nix")
        .arg("show-derivation")
        .arg("--recursive")
        .arg(format!("{}#{}", &job.flake_url, &attr_name))
        .output()
        .expect("failed to execute process");

    let show_output = String::from_utf8(show.stdout).expect("could not retrieve stdout");

    //log_command_output(show);

    let drvs: HashMap<String, Derivation> =
        serde_json::from_str(&show_output).expect("failed to parse derivation");

    println!("hash map = {:#?}", drvs);

    // make it possible to look up derivation paths by output path
    let drv_lookup: HashMap<&PathBuf, &String> =
        drvs.iter().flat_map(|(_k, v)| v.outputs_rev()).collect(); // .cloned?

    let validated_by: HashMap<&String, &String> = drvs
        .iter()
        .filter(|(_p, validator)| //: HashMap<&String, &String>
          validator.env.VERIFIES.is_some())
        .map(
            |(p, validator)| {
                let verifies: String = validator.env.VERIFIES.as_ref().unwrap().clone();
                (
                    drv_lookup.get(&PathBuf::from(verifies)).unwrap().clone(), // .cloned().into()).unwrap()
                    p,
                )
            }, // take output path from env value
               // look up drv path based on that
               // create a new map with the found drv path as the key and
               // the original drv path as the value
        )
        .collect();

    // finally when outputting the build results
    // before outputting a failure check if there
    // is an entry in the validated_by list
    // and if there is and this succeeds, then mark it green (verified by other drv)
    // and if there is and this failes, then mark it red (violates other drv)
    println!("validated_by = {:#?}", validated_by);

    JobInstantiation {
        to_build,
        results,
        skip_list,
    }
}
