//! System tables embedded from arcpy-generated GDB (ArcGIS Pro 3.5)
//! Using include_bytes! for each template file.

use std::path::Path;

macro_rules! write_template {
    ($gdb:expr, $num:tt) => {
        {
            let table = include_bytes!(concat!("../../templates/a000000", $num, ".gdbtable"));
            let tablx = include_bytes!(concat!("../../templates/a000000", $num, ".gdbtablx"));
            std::fs::write($gdb.join(concat!("a000000", $num, ".gdbtable")), table)
                .map_err(|e| format!("写 a000000{}.gdbtable 失败: {}", $num, e))?;
            std::fs::write($gdb.join(concat!("a000000", $num, ".gdbtablx")), tablx)
                .map_err(|e| format!("写 a000000{}.gdbtablx 失败: {}", $num, e))?;
        }
    };
}

pub fn write_all_templates(gdb: &Path) -> Result<(), String> {
    write_template!(gdb, "02");
    write_template!(gdb, "03");
    write_template!(gdb, "05");
    write_template!(gdb, "06");
    write_template!(gdb, "07");
    write_template!(gdb, "09");
    write_template!(gdb, "0a");
    Ok(())
}
