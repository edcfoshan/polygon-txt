// Debug test: generate actual SHP and GDB output and verify
use std::collections::HashMap;

#[test]
fn debug_generate_and_verify_shp() {
    // Read test TXT
    let txt_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("test_data")
        .join("44120000072.txt");
    
    if !txt_path.exists() {
        eprintln!("Test TXT not found at {:?}", txt_path);
        return;
    }
    
    let text = std::fs::read_to_string(&txt_path).unwrap();
    let parsed = jisig_bpoint_converter_lib::txt::parse_txt(&text);
    println!("Parsed {} plots", parsed.plots.len());
    
    // Convert to SHP data
    let mut geometries = Vec::new();
    let mut attributes = Vec::new();
    for plot in &parsed.plots {
        let coords: Vec<(f64, f64)> = plot.coords.iter().map(|&(y, x)| (x, y)).collect();
        if coords.len() >= 3 {
            geometries.push(coords);
            let mut attr = HashMap::new();
            attr.insert("DKMC".to_string(), plot.name.clone());
            attr.insert("DKBH".to_string(), String::new());
            attr.insert("MJ".to_string(), plot.area.clone());
            attr.insert("DKYT".to_string(), plot.use_field.clone());
            attr.insert("TFH".to_string(), plot.tfh.clone());
            attr.insert("DLBM".to_string(), plot.dlbm.clone());
            attributes.push(attr);
        }
    }
    println!("Geometries: {}, Attributes: {}", geometries.len(), attributes.len());
    assert_eq!(geometries.len(), attributes.len());
    
    // Write to temp dir
    let out_dir = std::env::temp_dir().join("jisig_debug_test");
    std::fs::create_dir_all(&out_dir).unwrap();
    
    let result = jisig_bpoint_converter_lib::shp::write_shapefile(
        &out_dir, "debug_output", &geometries, &attributes,
        "2000国家大地坐标系", "3", "38"
    );
    
    match result {
        Ok(files) => {
            println!("Output files: {:?}", files);
            // Check DBF header
            let dbf_path = out_dir.join("debug_output.dbf");
            if dbf_path.exists() {
                let data = std::fs::read(&dbf_path).unwrap();
                println!("DBF size: {} bytes", data.len());
                println!("DBF byte 0 (version): 0x{:02x}", data[0]);
                println!("DBF bytes 1-3 (date): {:02x} {:02x} {:02x}", data[1], data[2], data[3]);
                let nrec = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let hlen = u16::from_le_bytes([data[8], data[9]]);
                let rlen = u16::from_le_bytes([data[10], data[11]]);
                println!("DBF: records={}, header_len={}, record_len={}", nrec, hlen, rlen);
                
                // Dump bytes 12-31 (remaining header)
                print!("DBF bytes 12-31: ");
                for i in 12..32 {
                    print!("{:02x} ", data[i]);
                }
                println!();
                println!("DBF byte 29 (lang driver): 0x{:02x} (should be 0x7C)", data[29]);
                
                // Parse field descriptors
                let nfields = (hlen as usize - 33) / 32;
                println!("Fields: {}", nfields);
                for fi in 0..nfields {
                    let off = 32 + fi*32;
                    let name = String::from_utf8_lossy(&data[off..off+11])
                        .trim_end_matches('\0').to_string();
                    let ftype = data[off + 11];
                    let flen = data[off + 16];
                    let fdec = data[off + 17];
                    let foff = u32::from_le_bytes([data[off+12], data[off+13], data[off+14], data[off+15]]);
                    println!("  Field[{}]: '{}' type={} len={} dec={} offset_in_record={}",
                        fi, name, ftype as char, flen, fdec, foff);
                }
            }
            
            // Check SHP header
            let shp_path = out_dir.join("debug_output.shp");
            if shp_path.exists() {
                let data = std::fs::read(&shp_path).unwrap();
                let magic = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let flen = i32::from_be_bytes([data[24], data[25], data[26], data[27]]);
                let stype = i32::from_le_bytes([data[32], data[33], data[34], data[35]]);
                println!("SHP: magic={}, file_len_16bit={}, shape_type={}", magic, flen, stype);
            }
            
            // Check SHX
            let shx_path = out_dir.join("debug_output.shx");
            if shx_path.exists() {
                let data = std::fs::read(&shx_path).unwrap();
                let magic = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let flen = i32::from_be_bytes([data[24], data[25], data[26], data[27]]);
                let nrecs = (flen - 50) / 8;
                println!("SHX: magic={}, file_len_16bit={}, index_records={}", magic, flen, nrecs);
            }
        },
        Err(e) => {
            eprintln!("ERROR: {}", e);
        }
    }
}

#[test]
fn debug_generate_and_verify_gdb() {
    let txt_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("test_data")
        .join("44120000072.txt");
    
    if !txt_path.exists() {
        eprintln!("Test TXT not found at {:?}", txt_path);
        return;
    }
    
    let text = std::fs::read_to_string(&txt_path).unwrap();
    let parsed = jisig_bpoint_converter_lib::txt::parse_txt(&text);
    println!("Parsed {} plots", parsed.plots.len());
    
    // Convert to SHP data
    let mut geometries = Vec::new();
    let mut attributes = Vec::new();
    for plot in &parsed.plots {
        let coords: Vec<(f64, f64)> = plot.coords.iter().map(|&(y, x)| (x, y)).collect();
        if coords.len() >= 3 {
            geometries.push(coords);
            let mut attr = HashMap::new();
            attr.insert("DKMC".to_string(), plot.name.clone());
            attr.insert("DKBH".to_string(), String::new());
            attr.insert("MJ".to_string(), plot.area.clone());
            attr.insert("DKYT".to_string(), plot.use_field.clone());
            attr.insert("TFH".to_string(), plot.tfh.clone());
            attr.insert("DLBM".to_string(), plot.dlbm.clone());
            attributes.push(attr);
        }
    }
    println!("Geometries: {}, Attributes: {}", geometries.len(), attributes.len());
    
    let out_dir = std::env::temp_dir().join("jisig_debug_gdb");
    std::fs::create_dir_all(&out_dir).unwrap();
    
    let fields: Vec<(String, String, u8, u32)> = vec![
        ("DKMC".into(), "地块名称".into(), 4u8, 50u32),
        ("DKBH".into(), "地块编号".into(), 4u8, 30u32),
        ("MJ".into(), "面积".into(), 3u8, 14u32),
        ("DKYT".into(), "用途".into(), 4u8, 50u32),
        ("TFH".into(), "图幅号".into(), 4u8, 20u32),
        ("DLBM".into(), "地类编码".into(), 4u8, 10u32),
    ];
    
    let mut crs_info = HashMap::new();
    crs_info.insert("c".to_string(), "2000国家大地坐标系".to_string());
    crs_info.insert("b".to_string(), "3".to_string());
    crs_info.insert("z".to_string(), "38".to_string());
    
    match jisig_bpoint_converter_lib::gdb::write_gdb_output(
        &out_dir, "debug_gdb", &fields, &attributes, &geometries, &crs_info
    ) {
        Ok(files) => {
            println!("GDB output: {:?}", files);
            let gdb_dir = out_dir.join("debug_gdb.gdb");
            assert!(gdb_dir.exists(), "GDB directory should exist");
            
            // Check essential GDB files
            let markers = ["gdb", "a00000001.gdbtable", "a00000002.gdbtable",
                          "a00000003.gdbtable", "a00000004.gdbtable"];
            for m in &markers {
                let p = gdb_dir.join(m);
                println!("  {}: {}", m, if p.exists() { "EXISTS" } else { "MISSING!" });
            }
            
            // Check timestamps table binary format
            let ts_path = gdb_dir.join("a00000003.gdbtable");
            if ts_path.exists() {
                let d = std::fs::read(&ts_path).unwrap();
                println!("spTimestamps: {} bytes", d.len());
                println!("  Header magic: {:02x}{:02x}{:02x}{:02x}", d[0], d[1], d[2], d[3]);
                let nfields = u16::from_le_bytes([d[40], d[41]]);
                println!("  num_fields: {}", nfields);
                let nrows = u32::from_le_bytes([d[44], d[45], d[46], d[47]]);
                println!("  num_rows: {}", nrows);
            }
            
            // Check GDB_Items table
            let items_path = gdb_dir.join("a00000004.gdbtable");
            if items_path.exists() {
                let d = std::fs::read(&items_path).unwrap();
                println!("GDB_Items: {} bytes", d.len());
                let nfields = u16::from_le_bytes([d[40], d[41]]);
                println!("  num_fields: {}", nfields);
                let nrows = u32::from_le_bytes([d[44], d[45], d[46], d[47]]);
                println!("  num_rows: {}", nrows);
            }
            
            // Try to re-read GDB to verify it's self-consistent
            match jisig_bpoint_converter_lib::gdb::read_gdb(&gdb_dir) {
                Ok(info) => {
                    println!("Re-read GDB OK: {} layers", info.layers.len());
                    for l in &info.layers {
                        println!("  Layer '{}': {} features, {} fields", 
                            l.name, l.num_features, l.field_names.len());
                    }
                },
                Err(e) => {
                    eprintln!("Re-read GDB FAILED: {}", e);
                }
            }
        },
        Err(e) => {
            eprintln!("GDB ERROR: {}", e);
            panic!("GDB write failed: {}", e);
        }
    }
}
