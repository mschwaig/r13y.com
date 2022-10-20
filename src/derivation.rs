use serde_json;

use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
    process::Command,
};

use crate::messages::Attr;

// we only deserialize the part of Env we care about
#[derive(Deserialize, Debug)]
//#[serde_as]
pub(crate) struct Env {
    //#[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
    //VERIFIES: Option<HashSet<String>>
    pub(crate) VERIFIES: Option<String>, // TODO: try serde deserialize_with
}

#[derive(Deserialize, Debug)]
pub struct Derivation {
    outputs: HashMap<String, HashMap<String, PathBuf>>,
    // inputSrcs,
    // inputDrvs,
    // platform,
    // builder,
    // args,
    // env: HashMap<String, String>,
    pub(crate) env: Env,
}

impl Derivation {
    pub fn parse(drv: &Path) -> Result<Derivation, DerivationParseError> {
        let mut drvs = Derivation::parse_many(&[drv])?;
        let key = drv.to_str().unwrap();

        match drvs.remove(key) {
            Some(parsed) => Ok(parsed),
            None => Err(DerivationParseError::NotInResult),
        }
    }

    pub fn parse_many(drvs: &[&Path]) -> Result<HashMap<String, Derivation>, DerivationParseError> {
        debug!("Parsing derivations: {:#?}", &drvs);
        let show = Command::new("nix")
            .arg("show-derivation")
            .args(drvs.iter())
            .output()?;
        for line in show.stderr.lines() {
            debug!("parse derivation stderr: {:?}", line);
        }
        Ok(serde_json::from_slice(&show.stdout)?)
    }

    pub fn outputs(&self) -> HashMap<&String, &PathBuf> {
        self.outputs
            .iter()
            .map(|(name, submap)| (name, submap.get("path")))
            .filter_map(|(name, path)| path.map(|p| (name, p)))
            .collect()
    }

    pub fn output_to_drv_path_map(&self, drv_path : &String) -> HashMap<&PathBuf, String> {
        self.outputs
            .iter()
            .map(|(name, submap)| (name, submap.get("path")))
            .filter_map(|(name, path)| path.map(|p| (p, drv_path.clone())))
            .collect()
    }

    pub fn drvs_in_closure_are_validated_by(flake_url : &String, attr : &Attr) -> HashMap<String, String> {
        let attr_name = attr.join(".");

        info!("Evaluating {}#{}", flake_url, attr_name);
        let show = Command::new("nix")
            .arg("show-derivation")
            .arg("--recursive")
            .arg(format!("{}#{}", flake_url, &attr_name))
            .output()
            .expect("failed to execute process");

        let show_output = String::from_utf8(show.stdout).expect("could not retrieve stdout");

        //log_command_output(show);

        let drvs: HashMap<String, Derivation> =
            serde_json::from_str(&show_output).expect("failed to parse derivation");

        info!("hash map = {:#?}", drvs);

        // make it possible to look up derivation paths by output path
        let drv_lookup: HashMap<&PathBuf, String> =
            drvs.iter().flat_map(|(k, v)| v.output_to_drv_path_map(k)).collect(); // .cloned?

        let validated_by: HashMap<String, String> = drvs
            .iter()
            .filter(|(p, validator)|
              validator.env.VERIFIES.is_some())
            .map(
                |(p, validator)| {
                    let verifies: String = validator.env.VERIFIES.as_ref().unwrap().clone();
                    (
                        drv_lookup.get(&PathBuf::from(verifies)).unwrap().clone(), // .cloned().into()).unwrap()
                        p.clone(),
                    )
                }, // take output path from env value
                   // look up drv path based on that
                   // create a new map with the found drv path as the key and
                   // the original drv path as the value
            )
            .collect();

        info!("drv_lookup = {:#?}", drv_lookup);


        // finally when outputting the build results
        // before outputting a failure check if there
        // is an entry in the validated_by list
        // and if there is and this succeeds, then mark it green (verified by other drv)
        // and if there is and this failes, then mark it red (violates other drv)
        warn!("validated_by = {:#?}", validated_by);

        return validated_by;
    }
}

#[derive(Debug)]
pub enum DerivationParseError {
    Io(std::io::Error),
    JsonDecode(serde_json::Error),
    NotInResult,
}

impl From<serde_json::Error> for DerivationParseError {
    fn from(e: serde_json::Error) -> Self {
        DerivationParseError::JsonDecode(e)
    }
}

impl From<std::io::Error> for DerivationParseError {
    fn from(e: std::io::Error) -> Self {
        DerivationParseError::Io(e)
    }
}
