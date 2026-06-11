//! Generate a test GDB for arcpy verification
use std::collections::HashMap;
use std::path::Path;

fn main() {
    let out_dir = Path::new(r"C:\Users\Administrator\AppData\Local\Temp\arcpy_verify");
    let fields: Vec<(String, String, u8, u32)> = vec![
        ("DKMC".into(), "地块名称".into(), 4u8, 50u32),
        ("DKBH".into(), "地块编号".into(), 4u8, 30u32),
        ("MJ".into(), "面积".into(), 3u8, 14u32),
        ("DKYT".into(), "用途".into(), 4u8, 50u32),
        ("TFH".into(), "图幅号".into(), 4u8, 20u32),
        ("DLBM".into(), "地类编码".into(), 4u8, 10u32),
    ];
    
    let mut attributes = Vec::new();
    let mut attr = HashMap::new();
    attr.insert("DKMC".to_string(), "测试地块".to_string());
    attr.insert("MJ".to_string(), "1234.56".to_string());
    attr.insert("DKYT".to_string(), "耕地".to_string());
    attributes.push(attr);
    
    let geometries = vec![
        vec![
            (38383243.971, 2582988.976),
            (38383261.067, 2582983.339),
            (38383048.719, 2582359.231),
            (38383061.719, 2582359.231),
            (38383243.971, 2582988.976),
        ]
    ];
    
    let mut crs = HashMap::new();
    crs.insert("c".to_string(), "2000国家大地坐标系".to_string());
    crs.insert("b".to_string(), "3".to_string());
    crs.insert("z".to_string(), "38".to_string());
    
    match jisig_bpoint_converter_lib::gdb::write_gdb_output(
        out_dir, "test_output", &fields, &attributes, &geometries, &crs
    ) {
        Ok(files) => {
            println!("GDB created:");
            for f in &files {
                println!("  {}", f);
            }
        }
        Err(e) => eprintln!("ERROR: {}", e),
    }
}
