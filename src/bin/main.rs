use log::debug;

use serde_json::Value;

use structopt::StructOpt;
use std::process::Command;

use r13y::{
    check::check,
    messages::{Attr, BuildRequest, BuildRequestV1},
    report::report,
};

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(long = "result-url")]
    result_url: Option<String>,

    #[structopt(subcommand)]
    mode: Mode,

    #[structopt(long = "max-cores", default_value = "3")]
    maximum_cores: u16,
    #[structopt(long = "max-cores-per-job", default_value = "1")]
    maximum_cores_per_job: u16,

    /// Which derivation from any flake to test.
    /// Format: `flake/rev#attr.path flake2/rev2#attr.path`.
    #[structopt(short = "f", long = "flake", parse(try_from_str = "parse_subset"))]
    subsets: (String, Attr),
}

#[derive(StructOpt, Debug)]
enum Mode {
    #[structopt(name = "check")]
    Check,
    #[structopt(name = "report")]
    Report,
}

fn parse_subset(s: &str) -> Result<(String, Attr), &'static str> {
    let mut comp = s.split('#');

    let subset = match comp.next() {
        Some(x) => x,
        None => return Err("no subset specifier"),
    };

    if let Some(attr) = comp.next() {
        let attr_path = attr.split('.').map(str::to_owned).collect();
        Ok((subset.to_string(), attr_path))
    } else {
        Err("Empty attribute specifier")
    }
}

fn main() {
    env_logger::init();

    let opt = Opt::from_args();

    debug!("Using options: {:#?}", opt);

    let resolve = Command::new("nix")
        .arg("flake")
        .arg("metadata")
        .arg("--json")
        .arg(opt.subsets.0)
        .output()
        .expect("failed to execute process");

    let resolve_output = String::from_utf8(resolve.stdout).unwrap();

    debug!("Resolve output: {:#?}", resolve_output);

    let v: Value = serde_json::from_str(&resolve_output).expect("failed to parse flake metadata");
    // some strange dance to get rid of the quotes
    let flake_url = v["resolvedUrl"].as_str().unwrap().to_string();
    let revision = v["revision"].as_str().unwrap().to_string();
    let nar_hash = v["locked"]["narHash"].as_str().unwrap().to_string();

    debug!("flake_url: {flake_url}, revision: {revision}, nar_hash: {nar_hash}");

    let instruction = BuildRequest::V1(BuildRequestV1 {
        flake_url,
        revision,
        nar_hash,
        result_url: opt.result_url.unwrap_or_else(|| String::from("bogus")),
        attr: opt.subsets.1,
    });

    debug!("Using instruction: {:#?}", instruction);

    match opt.mode {
        Mode::Check => check(instruction, opt.maximum_cores, opt.maximum_cores_per_job),
        Mode::Report => report(instruction),
    }
}
