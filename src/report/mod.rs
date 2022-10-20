use chrono::Utc;

use crate::{
    cas::ContentAddressedStorage,
    derivation::Derivation,
    diffoscope::Diffoscope,
    eval::{eval, JobInstantiation},
    messages::{BuildRequest, BuildStatus},
};

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    collections::HashMap,
};

pub fn report(instruction: BuildRequest) {
    let job = match instruction {
        BuildRequest::V1(ref req) => req.clone(),
    };

    let JobInstantiation {
        to_build, results, ..
    } = eval(instruction.clone());

    let tmpdir = PathBuf::from("./tmp/");
    let report_dir = PathBuf::from("./report/");
    fs::create_dir_all(&report_dir).unwrap();
    let diff_dir = PathBuf::from("./report/diff");
    fs::create_dir_all(&diff_dir).unwrap();
    let mut html = File::create(report_dir.join("index.html")).unwrap();

    let read_cas = ContentAddressedStorage::new(tmpdir.clone());
    let write_cas = ContentAddressedStorage::new(report_dir.clone().join("cas"));
    let diffoscope = Diffoscope::new(write_cas.clone());
    let mut total = 0;
    let mut reproducible = 0;
    let mut unreproducible_list: Vec<String> = vec![];
    let mut unchecked_list: Vec<String> = vec![];
    let mut unchecked = 0;
    let mut first_failed: Vec<&String> = vec![];

    let validated_by = Derivation::drvs_in_closure_are_validated_by(&job.flake_url, &job.attr);

    let attr_name = job.attr.join(".");

    let results = results.iter().map(|x| (&x.drv, x)).collect::<HashMap<_,_>>();

    for response in results.values().into_iter().filter(|response| {
        (match response.request {
            BuildRequest::V1(ref req) => req.nar_hash == job.nar_hash,
        }) && to_build.contains(&PathBuf::from(&response.drv))
    }) {
        total += 1;
        match &response.status {
            BuildStatus::Reproducible => {
                reproducible += 1;
            }
            BuildStatus::FirstFailed => {
                first_failed.push(&response.drv);
            }
            BuildStatus::SecondFailed => {
                unchecked += 1;
                let unchecked_line = match validated_by.get(&response.drv) {
                    Some(x) => {
                        let y = results.get(x).unwrap();
                        match y.status {
                            BuildStatus::Reproducible => format!(
                                "<li><code>{}</code> (verified by <code>{}</code>)</li>", response.drv, y.drv),
                            _ => format!("<li><code>{}</code> (failed verification by <code>{}</code>)</li>", response.drv, y.drv)
                        }
                    },
                    None => format!("<li><code>{}</code></li>", response.drv)
                };
                unchecked_list.push(unchecked_line);

            }
            BuildStatus::Unreproducible(hashes) => {
                let parsed_drv = Derivation::parse(&Path::new(&response.drv)).unwrap();

                unreproducible_list.push(format!("<li><code>{}</code></li>", response.drv));
                for (output, (hash_a, hash_b)) in hashes.iter() {
                    if let Some(output_path) = parsed_drv.outputs().get(output) {
                        let dest_name = format!("{}-{}.html", hash_a, hash_b);
                        let dest = diff_dir.join(&dest_name);

                        if dest.exists() {
                            // ok
                        } else {
                            println!(
                                "Diffing {}'s {}: {} vs {}",
                                response.drv, output, hash_a, hash_b
                            );

                            let cas_a = read_cas.str_to_id(hash_a).unwrap();
                            let cas_b = read_cas.str_to_id(hash_b).unwrap();
                            let savedto = diffoscope
                                .nars(
                                    &output_path.file_name().unwrap().to_string_lossy(),
                                    &cas_a.as_path_buf(),
                                    &cas_b.as_path_buf(),
                                )
                                .unwrap();
                            println!("saved to: {}", savedto.display());
                            fs::copy(savedto, dest).unwrap();
                        }
                        unreproducible_list.push(format!(
                            "<li><a href=\"./diff/{}\">(diffoscope)</a> {}</li>",
                            dest_name, output
                        ));
                    } else {
                        println!("Diffing {} but no output named {}", response.drv, output);
                        // <li><a href="./diff/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv.html">(diffoscope)</a> <a href="./nix/store/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv">(drv)</a> <code>/nix/store/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv</code></li>
                    }
                }
                unreproducible_list.push("</ul></li>".to_string());

                println!("{:#?}", hashes);
            }
        }
    }

    if !first_failed.is_empty() {
        panic!("{} are unchecked:\n{:#?}", first_failed.len(), first_failed);
    }

    html.write_all(
        format!(
            include_str!("./template.html"),
            reproduced = reproducible,
            unchecked = unchecked,
            total = total,
            percent = format!("{:.*}%", 2, 100.0 * (reproducible as f64 / total as f64)),
            revision = job.revision,
            now = Utc::now().to_string(),
            unreproduced_list = unreproducible_list.join("\n"),
            unchecked_list = unchecked_list.join("\n"),
            attr_name = attr_name,
        )
        .as_bytes(),
    )
    .unwrap();

    File::create(report_dir.join("metrics"))
        .unwrap()
        .write_all(format!(
"
# HELP r13y_check_revision Check's nixpkgs revision
# TYPE r13y_check_revision counter
r13y_check_revision{{revision=\"{revision}\"}} 1
# HELP r13y_check_time_seconds Time of the latest check
# TYPE r13y_check_time_seconds counter
r13y_check_time_seconds {time}
# HELP r13y_paths_checked Number of paths checked in the latest check
# TYPE r13y_paths_checked gauge
r13y_paths_count {total}
# HELP r13y_path_status_counts Number of paths in each status
# TYPE r13y_path_status_counts gauge
r13y_path_status_count{{status=\"reproducible\"}} {reproducible}
r13y_path_status_count{{status=\"unreproducible\"}} {unreproducible}
r13y_path_status_count{{status=\"unchecked\"}} {unchecked}

",
            revision = job.revision,
            time = Utc::now().timestamp(),
            total = total,
            reproducible = reproducible,
            unreproducible = total - reproducible,
            unchecked = unchecked,
        ).as_bytes())
        .unwrap();


}
