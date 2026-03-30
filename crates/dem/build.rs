use minijinja::{context, Environment};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
struct Fixture {
    events: Vec<EventFixture>,
    event_manager: Option<EventManagerFixture>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EventFixture {
    id: u16,
    name: String,
    calib: CalibFixture,
    nvm: NvmFixture,
}

#[derive(Debug, Deserialize, Serialize)]
struct CalibFixture {
    step_up: i16,
    step_down: i16,
    debounce_type: String,
    debounce_behavior: String,
    confirmation_threshold: u8,
    aging_threshold: u8,
    priority: u8,
}

#[derive(Debug, Deserialize, Serialize)]
struct NvmFixture {
    raw_status: u8,
    occurence_cntr: u8,
    aging_cycles: u8,
    confirmation_cycles: u8,
}

#[derive(Debug, Deserialize, Serialize)]
struct EventManagerFixture {}

fn sanitize_module_name(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_alphanumeric() || c == '_' {
            result.push(c);
        } else if i > 0 {
            result.push('_');
        }
    }
    if result
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("f_{}", result)
    } else {
        result
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_config.rs");

    let fixtures_dir = Path::new("tests/fixtures");
    let templates_dir = Path::new("templates");

    let mut fixtures_data = Vec::new();

    if fixtures_dir.exists() {
        if let Ok(entries) = fs::read_dir(fixtures_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(fixture) = serde_yaml::from_str::<Fixture>(&content) {
                            let stem = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("config");
                            let module_name = sanitize_module_name(stem);

                            fixtures_data.push(serde_json::json!({
                                "source_file": path.to_string_lossy(),
                                "module_name": module_name,
                                "events": fixture.events,
                                "event_manager": fixture.event_manager,
                            }));
                        }
                    }
                }
            }
        }
    }

    let template_path = templates_dir.join("generated_dem_config.rs.j2");
    let template_content =
        fs::read_to_string(&template_path).expect("Failed to read template file");

    let mut env = Environment::new();
    env.add_template("generated_dem_config.rs.j2", &template_content)
        .expect("Failed to add template");

    let template = env
        .get_template("generated_dem_config.rs.j2")
        .expect("Failed to get template");

    let rendered = template
        .render(context!(fixtures => fixtures_data))
        .expect("Failed to render template");

    fs::write(&dest_path, rendered).unwrap();
    println!("cargo:rerun-if-changed=tests/fixtures/");
    println!("cargo:rerun-if-changed=src/data/");
    println!("cargo:rerun-if-changed=templates/");
}
