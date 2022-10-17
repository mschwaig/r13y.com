use serde_json;

use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
    process::Command,
};

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

    pub fn outputs_rev(&self) -> HashMap<&PathBuf, &String> {
        self.outputs
            .iter()
            .map(|(name, submap)| (name, submap.get("path")))
            .filter_map(|(name, path)| path.map(|p| (p, name)))
            .collect()
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
