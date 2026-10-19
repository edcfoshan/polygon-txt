use clap::{Args, Parser, Subcommand};
use jisig_bpoint_converter_lib::{config::Config, convert, gdb, geometry, output, shp, txt};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "jisigg-cli",
    version,
    about = "极思G界址点互转工具 CLI — SHP/GDB ↔ TXT",
    long_about = "原生离线转换。复杂选项使用桌面版导出的 JSON 配置；命令行参数优先。日志写 stderr，--json 结果写 stdout。"
)]
struct Cli {
    #[arg(long, global = true, help = "输出供程序读取的 JSON")]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 检查 SHP/GDB/TXT，列出字段、图层和数量
    Inspect {
        #[arg(long, required = true)]
        input: Vec<PathBuf>,
    },
    /// 配置校验与默认配置
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// SHP 或 GDB 面要素转界址点 TXT
    ShpToTxt(ConvertArgs),
    /// 界址点 TXT 转 SHP（包括 SHX/DBF/PRJ/CPG）
    TxtToShp(ConvertArgs),
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// 校验桌面版/CLI 共用的配置文件
    Validate { file: PathBuf },
    /// 输出可直接保存为文件的完整默认 JSON 配置
    Default,
}

#[derive(Args)]
struct ConvertArgs {
    #[arg(long, required = true, help = "输入文件，可重复；GDB 仅接受单个目录")]
    input: Vec<PathBuf>,
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, help = "桌面版导出的 JSON 配置；省略则用内置默认配置")]
    config: Option<PathBuf>,
    #[arg(long, value_parser = ["one_to_one", "split_by_plot", "merge_all"])]
    output_mode: Option<String>,
    #[arg(long, help = "拆分输出的命名字段；字段缺失沿用已有序号兜底")]
    filename_field: Option<String>,
    #[arg(long, value_parser = ["utf8", "gbk"], help = "仅面→TXT")]
    encoding: Option<String>,
    #[arg(long, help = "GDB 要素类名，可重复；省略则处理所有面图层")]
    layer: Vec<String>,
    #[arg(long, help = "仅面→TXT，选择 SOURCE:INDEX（均从 0 开始），可重复")]
    plot: Vec<String>,
    #[arg(long, help = "只在内存生成并检查全部结果，不创建目录或文件")]
    dry_run: bool,
    #[arg(long, help = "明确允许覆盖已存在的同名文件")]
    overwrite: bool,
}

fn load_config(path: Option<&PathBuf>) -> Result<Config, String> {
    match path {
        Some(p) => Config::parse(
            &std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?,
        ),
        None => Ok(Config::default()),
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

fn inspect(path: &Path) -> Result<Value, String> {
    match extension(path).as_str() {
        "shp" => {
            let (info, _) = shp::read_shp_file_group(path)?;
            if !geometry::is_polygon_geometry_type(&info.shape_type) {
                return Err(format!("{} 不是面要素", path.display()));
            }
            Ok(
                json!({"path":path,"type":"shp","fields":info.field_names,"feature_count":info.num_features,"crs":info.crs_info}),
            )
        }
        "gdb" => {
            let layers = gdb::read_gdb_catalog(path)?;
            Ok(json!({"path":path,"type":"gdb","layers":layers}))
        }
        "txt" => {
            let parsed = txt::parse_txt(&txt::read_text_file(path)?);
            if parsed.plots.is_empty() {
                return Err(format!("{} 没有可解析地块", path.display()));
            }
            Ok(
                json!({"path":path,"type":"txt","plot_count":parsed.plots.len(),"header":parsed.attrs,
                "plots":parsed.plots.iter().enumerate().map(|(index,p)| json!({"index":index,"name":p.name,"fid":p.fid,"point_count":p.coords.len()})).collect::<Vec<_>>()}),
            )
        }
        _ => Err(format!(
            "不支持的输入：{}；仅接受 SHP/GDB/TXT",
            path.display()
        )),
    }
}

fn check_shp_inputs(args: &ConvertArgs, config: &Config) -> Result<(bool, Vec<usize>), String> {
    let is_gdb = args.input.len() == 1 && extension(&args.input[0]) == "gdb";
    let mut errors = Vec::new();
    let mut counts = Vec::new();
    if is_gdb {
        let catalog = gdb::read_gdb_catalog(&args.input[0])?;
        for name in &args.layer {
            if !catalog
                .iter()
                .any(|l| &l.name == name && geometry::is_polygon_geometry_type(&l.geometry_type))
            {
                errors.push(format!("找不到面要素类：{}", name));
            }
        }
        let mut layers: Vec<_> = catalog
            .iter()
            .filter(|l| {
                geometry::is_polygon_geometry_type(&l.geometry_type)
                    && (args.layer.is_empty() || args.layer.contains(&l.name))
            })
            .collect();
        if !args.layer.is_empty() {
            layers.sort_by_key(|l| args.layer.iter().position(|n| n == &l.name).unwrap());
        }
        if layers.is_empty() {
            errors.push("没有可转换的面要素类".into());
        }
        for layer in layers {
            counts.push(layer.num_features.max(0) as usize);
            if let Err(e) = config.check_fields(&layer.field_names, &layer.name) {
                errors.push(e);
            }
        }
    } else {
        if !args.layer.is_empty() {
            errors.push("--layer 仅用于单个 GDB 输入".into());
        }
        for path in &args.input {
            if extension(path) != "shp" {
                errors.push(format!("面→TXT 需要 SHP 或单个 GDB：{}", path.display()));
                continue;
            }
            match shp::read_shp_file_group(path) {
                Ok((info, features)) => {
                    counts.push(features.len());
                    if !geometry::is_polygon_geometry_type(&info.shape_type) || features.is_empty()
                    {
                        errors.push(format!("{} 不包含可转换的面要素", path.display()));
                    }
                    if let Err(e) = config.check_fields(&info.field_names, &info.name) {
                        errors.push(e);
                    }
                }
                Err(e) => errors.push(format!("{}: {}", path.display(), e)),
            }
        }
    }
    if errors.is_empty() {
        Ok((is_gdb, counts))
    } else {
        Err(errors.join("\n"))
    }
}

fn convert(args: &ConvertArgs, to_txt: bool) -> Result<Value, String> {
    let mut config = load_config(args.config.as_ref())?;
    if to_txt {
        let o = &mut config.shp_to_txt.options;
        if let Some(v) = &args.output_mode {
            o.output_mode = v.clone();
        }
        if let Some(v) = &args.filename_field {
            o.filename_field = v.clone();
        }
        if let Some(v) = &args.encoding {
            o.txt_encoding = v.clone();
        }
    } else {
        if !args.layer.is_empty() || !args.plot.is_empty() || args.encoding.is_some() {
            return Err("TXT→SHP 不接受 --layer、--plot 或 --encoding".into());
        }
        if let Some(v) = &args.output_mode {
            config.txt_to_shp.output_mode = v.clone();
        }
        if let Some(v) = &args.filename_field {
            config.txt_to_shp.filename_field = v.clone();
        }
    }
    config.validate()?;
    let mut ancestor = Some(args.output_dir.as_path());
    while let Some(path) = ancestor {
        if path.exists() {
            if !path.is_dir() {
                return Err(format!("输出路径的父级不是目录：{}", path.display()));
            }
            break;
        }
        ancestor = path.parent();
    }
    let (is_gdb, counts) = if to_txt {
        check_shp_inputs(args, &config)?
    } else {
        let mut errors = Vec::new();
        for path in &args.input {
            if extension(path) != "txt" {
                errors.push(format!("TXT→SHP 只接受 TXT：{}", path.display()));
                continue;
            }
            match txt::read_text_file(path) {
                Ok(text) => {
                    let parsed = txt::parse_txt(&text);
                    if parsed.plots.is_empty() || parsed.plots.iter().any(|p| p.coords.len() < 3) {
                        errors.push(format!("{} 包含空地块或没有可解析地块", path.display()));
                    }
                    let mut warnings = Vec::new();
                    if convert::resolve_txt_zone(
                        convert::extract_zone_from_coords(&parsed.plots),
                        parsed.attrs.get("带号").map(String::as_str),
                        &config.shp_to_txt.header.attr("带号"),
                        &path.display().to_string(),
                        &mut warnings,
                    )
                    .is_none()
                    {
                        errors.push(format!(
                            "{} 无法确定带号，请在配置表头中填写",
                            path.display()
                        ));
                    }
                }
                Err(e) => errors.push(format!("{}: {}", path.display(), e)),
            }
        }
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        (false, Vec::new())
    };
    if !args.plot.is_empty() {
        let pairs = args
            .plot
            .iter()
            .map(|p| {
                let (a, b) = p
                    .split_once(':')
                    .ok_or_else(|| "--plot 格式必须为 SOURCE:INDEX".to_string())?;
                Ok([
                    a.parse::<usize>().map_err(|_| "源下标无效")?,
                    b.parse::<usize>().map_err(|_| "地块下标无效")?,
                ])
            })
            .collect::<Result<Vec<_>, String>>()?;
        let invalid: Vec<_> = pairs
            .iter()
            .filter(|[source, index]| counts.get(*source).is_none_or(|count| index >= count))
            .map(|[source, index]| format!("{}:{}", source, index))
            .collect();
        if !invalid.is_empty() {
            return Err(format!("地块筛选下标不存在：{}", invalid.join("、")));
        }
        config.shp_to_txt.options.plot_filter = Some(pairs);
    }
    let s = &config.shp_to_txt;
    let (result, files) = output::capture(|| {
        if to_txt {
            convert::convert_shp_to_txt(
                &args.input,
                if is_gdb { Some("gdb") } else { None },
                if is_gdb { Some(&args.input[0]) } else { None },
                &s.header,
                &s.fields,
                &s.options,
                &args.output_dir,
                if is_gdb && !args.layer.is_empty() {
                    Some(&args.layer)
                } else {
                    None
                },
            )
        } else {
            convert::convert_txt_to_shp(
                &args.input,
                &config
                    .txt_to_shp
                    .options(args.output_dir.display().to_string()),
                &s.header,
            )
        }
    })?;
    if files.is_empty() {
        return Err("转换没有生成可写出的文件，请检查输入和配置".into());
    }
    output::validate(&files)?;
    let conflicts = output::conflicts(&files);
    if !args.overwrite && !conflicts.is_empty() {
        return Err(format!(
            "输出文件已存在（使用 --overwrite 才可覆盖）：\n{}",
            conflicts.join("\n")
        ));
    }
    if !args.dry_run {
        output::commit(&files, args.overwrite)?;
    }
    Ok(
        json!({"success":true,"dry_run":args.dry_run,"message":if args.dry_run {"预检通过，未写入文件"} else {&result.message},
        "processed_count":result.processed_count,"output_files":files.iter().map(|(p,_)| p).collect::<Vec<_>>(),
        "output_bytes":files.iter().map(|(_,b)|b.len() as u64).sum::<u64>()}),
    )
}

fn run(cli: &Cli) -> Result<Value, String> {
    match &cli.command {
        Command::Inspect { input } => Ok(
            json!({"success":true,"sources":input.iter().map(|p| inspect(p)).collect::<Result<Vec<_>,_>>()?}),
        ),
        Command::Config {
            command: ConfigCommand::Default,
        } => Ok(serde_json::to_value(Config::default()).unwrap()),
        Command::Config {
            command: ConfigCommand::Validate { file },
        } => {
            let config = load_config(Some(file))?;
            Ok(json!({"success":true,"name":config.name,"schema_version":config.schema_version}))
        }
        Command::ShpToTxt(args) => convert(args, true),
        Command::TxtToShp(args) => convert(args, false),
    }
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if e.use_stderr() && std::env::args().any(|a| a == "--json") {
                println!("{}", json!({"success":false,"error":e.to_string()}));
                std::process::exit(2);
            }
            e.exit();
        }
    };
    match run(&cli) {
        Ok(value) => {
            if cli.json
                || matches!(
                    cli.command,
                    Command::Config {
                        command: ConfigCommand::Default
                    }
                )
            {
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            } else {
                println!(
                    "{}",
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("检查通过")
                );
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(error) => {
            if cli.json {
                println!("{}", json!({"success":false,"error":error}));
            } else {
                eprintln!("错误：{}", error);
            }
            std::process::exit(1);
        }
    }
}
