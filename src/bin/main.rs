use log::debug;

use serde_derive::Deserialize;
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
    flake_attr: FlakeAttr,
}

#[derive(StructOpt, Debug)]
enum Mode {
    #[structopt(name = "check")]
    Check,
    #[structopt(name = "report")]
    Report,
}

#[derive(Debug)]
struct FlakeAttr {
    flake : Flake,
    attr: Attr
}

#[derive(Deserialize, Debug)]
struct Flake {
    url : String,
    locked : Locked
}

#[derive(Deserialize, Debug)]
struct Locked {
    #[serde(rename = "narHash")]
    nar_hash : String
}

fn parse_subset(s: &str) -> Result<FlakeAttr, &'static str> {
    let mut comp = s.split('#');

    let subset = match comp.next() {
        Some(x) => x,
        None => return Err("no subset specifier"),
    };

    let attr_path = match comp.next() {
        Some (attr) => attr.split('.').map(str::to_owned).collect(),
        None => return Err("Empty attribute specifier")
    };

    let resolve = Command::new("nix")
        .arg("flake")
        .arg("metadata")
        .arg("--json")
        .arg(subset)
        .output()
        .expect("failed to execute process");

    let resolve_output = String::from_utf8(resolve.stdout).expect("could not retrieve stdout");

    debug!("Resolve output: {}", resolve_output);

    let mut flake: Flake = serde_json::from_str(&resolve_output).expect("failed to parse flake metadata");
    // sadly nar hashes cannot occur in file names as they are because they can contain /
    // so we replace that caracter and use that modified version everywhere
    flake.locked.nar_hash = flake.locked.nar_hash.replace("/", "\\");
    println!("deserialized = {:?}", flake);

    Ok(FlakeAttr{ flake, attr : attr_path })
}

fn main() {
    env_logger::init();

    let opt = Opt::from_args();

    debug!("Using options: {:#?}", opt);


    let instruction = BuildRequest::V1(BuildRequestV1 {
        flake_url: opt.flake_attr.flake.url,
        nar_hash : opt.flake_attr.flake.locked.nar_hash,
        result_url: opt.result_url.unwrap_or_else(|| String::from("bogus")),
        attr: opt.flake_attr.attr,
    });

    debug!("Using instruction: {:#?}", instruction);

    match opt.mode {
        Mode::Check => check(instruction, opt.maximum_cores, opt.maximum_cores_per_job),
        Mode::Report => report(instruction),
    }
}
