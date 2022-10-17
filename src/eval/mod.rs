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
        .arg(format!("{}#{}", &job.flake_url, &attr_name))
        .output()
        .expect("failed to execute process");

    let to_build = eval
        .stdout
        .lines()
        .filter_map(|line_result| {
            line_result
                .ok()
                .and_then(|line| line.ends_with(".drv").then_some(line.into()))
        })
        .collect();

    log_command_output(eval);

    JobInstantiation {
        to_build,
        results,
        skip_list,
    }
}
