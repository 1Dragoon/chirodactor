use std::{collections::BTreeSet, error::Error, fs::File, io::{Read, Write}};
use clap::Parser;
use indexmap::IndexSet;
use regex::Regex;
use serde_json::Value;

fn process_node(value: &mut Value, mapper: &mut IndexSet<String>, log: &mut BTreeSet<String>, phone_regex: &Regex) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        Value::Array(arr) => {
            for elem in arr {
                process_node(elem, mapper, log, phone_regex);
            }
        }
        Value::Object(ref mut obj) => {
            for (key, val) in obj {
                mutate_value(key, val, mapper, log, phone_regex);
                process_node(val, mapper, log, phone_regex);
            }
        }
    }
}

pub enum GuestimatedType {
    Phone,
    Email,
    Name,
    Dunno
}

fn mutate_value(key: &String, value: &mut Value, mapper: &mut IndexSet<String>, log: &mut BTreeSet<String>, phone_regex: &Regex) {
    if key.starts_with("data") || key.starts_with("display_name") || key.starts_with("sort_key") || key.starts_with("sort_key") || key.starts_with("account_name") || key.starts_with("sync") || key.starts_with("lookup") {
            if let Value::String(val) = value {
                let orig_val = val.to_string();
                *val = val.to_lowercase();
                if val == "true" || val == "false" {
                    return;
                }
                let mut t = GuestimatedType::Dunno;
                if val.contains('@') {
                    *val = val.split('@').next().unwrap().to_string();
                    t = GuestimatedType::Email;
                }
                // Flip "last, first"
                if val.contains(',') {
                    *val = val.split(',').rev().collect::<Vec<_>>().concat();
                    t = GuestimatedType::Name;
                }
                // Remove common symbols
                *val = val.trim().replace(['.', '\'', '\n', ' ', '+', '-', '(', ')'], "").replace("+1", "");
                if let Some(groups) = phone_regex.captures(val) {
                    // Strip the 1 off of us country code phone numbers
                    if let Some(us_coded_number) = groups.get(1) {
                        *val = us_coded_number.as_str().chars().skip(1).collect();
                        t = GuestimatedType::Phone;
                    } else if let Some(uncoded_us_number) = groups.get(2) {
                        *val = uncoded_us_number.as_str().to_string();
                        t = GuestimatedType::Phone;
                    }
                }
                mapper.insert(val.to_string());
                let prefix = match t {
                    GuestimatedType::Phone => "phone",
                    GuestimatedType::Email => "email",
                    GuestimatedType::Name => "name_",
                    GuestimatedType::Dunno => "dunno",
                };
                let new_value = Value::String(format!("{prefix}_{:0>4}", mapper.get_index_of(val).unwrap()));
                log.insert(format!("key: {key} value: {orig_val} -> {new_value}"));
                *value = new_value;
            }
            if let Value::Number(num) = value {
                let numstr = num.to_string();
                mapper.insert(numstr.clone());
                let new_value = Value::String(format!("number_{:0>4}", mapper.get_index_of(&numstr).unwrap()));
                log.insert(format!("key: {key} value: {value} -> {new_value}"));
                *value = new_value;
            }
        } else if let Value::String(_) = value {
            log.insert(format!(r#"Unchanged field "{key}" value: {value}"#));
        } else if let Value::Number(_) = value {
            log.insert(format!(r#"Unchanged field "{key}" value: {value}"#));
        }
}

#[derive(Parser)]
#[clap(name = "chirodactor", about = "redact data from json, or something")]
struct Args {
    /// Path to your json file
    path: std::path::PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let phone_regex = Regex::new("(1[0-9]{10})|([0-9]{10})")?;
    let mut file = File::open(&args.path)?;
    let mut buffer = Vec::with_capacity(file.metadata()?.len() as _);
    let bytes = file.read_to_end(&mut buffer)?;
    println!("Read {bytes} bytes");
    let mut data = serde_json::from_slice::<Value>(&buffer)?;
    let mut mapper = IndexSet::new();
    let mut log = BTreeSet::new();

    process_node(&mut data, &mut mapper, &mut log, &phone_regex);

    for entry in log {
        println!("{entry}");
    }
    let output = serde_json::to_string_pretty(&data)?;
    let outfile_path = format!("{}-chirodacted.{}", args.path.file_stem().unwrap_or_default().to_string_lossy(), args.path.extension().unwrap_or_default().to_string_lossy());
    println!("{outfile_path:#?}");
    let mut outfile = File::create(outfile_path)?;
    outfile.write_all(output.as_bytes())?;
    Ok(())
}
