// 诊断：用本项目代码读取指定 GDB，打印真实错误 + 首尾坐标点用于对照 arcpy
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gdb_dir = if args.len() >= 2 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from(
            r"C:\Users\Administrator\Documents\txt与gdb互转\00测试数据\01自制测试数据\新建文件地理数据库.gdb",
        )
    };

    eprintln!("==== 读取 GDB: {} ====", gdb_dir.display());

    match jisig_bpoint_converter_lib::gdb::read_gdb(&gdb_dir) {
        Ok(info) => {
            println!("OK: {} 个图层", info.layers.len());
            for l in &info.layers {
                println!(
                    "  Layer '{}' geom={} features={} fields={:?}",
                    l.name, l.geometry_type, l.num_features, l.field_names
                );
            }
            for (li, feats) in info.all_features.iter().enumerate() {
                println!("图层 #{}: {} 个要素", li, feats.len());
                for (fi, f) in feats.iter().enumerate() {
                    println!(
                        "  feat[{}] points={} attrs={:?}",
                        fi,
                        f.points.len(),
                        f.attributes
                    );
                    if let (Some(a), Some(b)) = (f.points.first(), f.points.last()) {
                        println!("    first pt: (x={}, y={})", a.0, a.1);
                        println!("    last  pt: (x={}, y={})", b.0, b.1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("READ FAILED:");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
