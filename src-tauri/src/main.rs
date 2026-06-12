// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(smoke) = parse_smoke_args() {
        match jisig_bpoint_converter_lib::smoke::run_release_smoke(smoke) {
            Ok(report) => {
                println!("{}", report);
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        }
    }

    jisig_bpoint_converter_lib::run()
}

fn parse_smoke_args() -> Option<jisig_bpoint_converter_lib::SmokeTestConfig> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--smoke-test") {
        return None;
    }

    let txt_path = find_arg_value(&args, "--smoke-txt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("test_arcpy")
                .join("txt_output")
                .join("plot_000.txt")
        });
    let output_dir = find_arg_value(&args, "--smoke-output")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    Some(jisig_bpoint_converter_lib::SmokeTestConfig {
        txt_path,
        output_dir,
    })
}

fn find_arg_value(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find_map(|pair| if pair[0] == key { Some(pair[1].clone()) } else { None })
}
