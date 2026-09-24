// DBF 编码探测与读取一致性基线。
//
// 目的：S3 让 DBF 只读一次盘（字节与编码向下传递）。本文件把「编码判定结果」
// 与「手动解析结果」钉死，改数据流时任何解码漂移都会失败。
//
// 覆盖两条此前默认套件跑不到的路径：
//   1) 有 .cpg 与无 .cpg 必须得到相同结果（无 .cpg 时走字节探测）
//   2) 无 .cpg 的 GBK DBF 必须按 GBK 解码（该路径由手动解析器承担）

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::shp;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root").to_path_buf()
}

/// 手工构造最小 dBase III+ DBF：单个 C 字段 + n 条记录（字段值字节原样写入，
/// 便于构造非 UTF-8 的编码场景）
fn build_dbf(field_name: &str, field_len: usize, values: &[&[u8]]) -> Vec<u8> {
    let header_len = 32 + 32 + 1; // 文件头 + 1 个字段描述符 + 0x0D
    let record_len = 1 + field_len; // 删除标记 + 字段
    let mut out = Vec::new();
    out.push(0x03); // dBase III+
    out.extend_from_slice(&[24, 9, 24]); // 日期（y-1900, m, d）
    out.extend_from_slice(&(values.len() as u32).to_le_bytes()); // @4 记录数
    out.extend_from_slice(&(header_len as u16).to_le_bytes()); // @8 头长度
    out.extend_from_slice(&(record_len as u16).to_le_bytes()); // @10 记录长度
    out.extend_from_slice(&[0u8; 20]); // 保留区

    let mut name = [0u8; 11];
    let n = field_name.len().min(11);
    name[..n].copy_from_slice(&field_name.as_bytes()[..n]);
    out.extend_from_slice(&name);
    out.push(b'C'); // @+11 类型
    out.extend_from_slice(&[0u8; 4]); // 数据地址
    out.push(field_len as u8); // @+16 长度
    out.push(0); // 小数位
    out.extend_from_slice(&[0u8; 14]);
    out.push(0x0D); // 字段描述符结束

    for v in values {
        out.push(b' '); // 未删除
        let mut buf = vec![b' '; field_len];
        let m = v.len().min(field_len);
        buf[..m].copy_from_slice(&v[..m]);
        out.extend_from_slice(&buf);
    }
    out.push(0x1A); // EOF
    out
}

// ─── 有 .cpg / 无 .cpg 结果一致（入库 fixture，UTF-8）───

#[test]
fn dbf_read_same_with_and_without_cpg() {
    let src = repo_root().join("test_data").join("44120000072_0.dbf");
    assert!(src.exists(), "缺少入库 fixture：{}", src.display());
    let cpg = src.with_extension("cpg");
    assert!(cpg.exists(), "该 fixture 应带 .cpg（UTF-8）");

    let with_cpg = shp::read_dbf(&src).expect("读带 .cpg 的 DBF");

    let tmp = tempfile::tempdir().expect("tmp");
    let dst = tmp.path().join("no_cpg.dbf");
    std::fs::copy(&src, &dst).expect("复制 DBF");
    assert!(!dst.with_extension("cpg").exists(), "临时目录刻意不提供 .cpg");
    let without_cpg = shp::read_dbf(&dst).expect("读无 .cpg 的 DBF");

    assert_eq!(with_cpg.0, without_cpg.0, "字段名应与带 .cpg 时一致");
    assert_eq!(with_cpg.1, without_cpg.1, "记录应与带 .cpg 时一致");
    assert!(!with_cpg.0.is_empty(), "该 fixture 应有字段");
    assert!(!with_cpg.1.is_empty(), "该 fixture 应有记录");
}

// ─── 无 .cpg 的 GBK DBF 按 GBK 解码（此前只有本地专用用例覆盖）───

#[test]
fn gbk_dbf_without_cpg_decodes_as_gbk() {
    // 「地块名称」的 GBK 字节（PowerShell: GetEncoding('GBK').GetBytes('地块名称')）
    const GBK_NAME: &[u8] = &[0xB5, 0xD8, 0xBF, 0xE9, 0xC3, 0xFB, 0xB3, 0xC6];
    let data = build_dbf("NAME", 20, &[GBK_NAME]);

    let tmp = tempfile::tempdir().expect("tmp");
    let p = tmp.path().join("gbk.dbf");
    std::fs::write(&p, &data).expect("写 GBK DBF");
    assert!(!p.with_extension("cpg").exists(), "本用例刻意不提供 .cpg");

    let (names, records) = shp::read_dbf(&p).expect("读 GBK DBF");
    assert_eq!(names, vec!["NAME".to_string()], "字段名应解析出来");
    assert_eq!(records.len(), 1, "应读出一条记录");
    assert_eq!(records[0][0], "地块名称", "GBK 字节应被按 GBK 解码（而不是 UTF-8 误解码）");
}

// ─── 同一 GBK 数据带 .cpg=GBK 时应得到同样结果（显式声明路径）───

#[test]
fn gbk_dbf_with_gbk_cpg_matches_detection() {
    const GBK_NAME: &[u8] = &[0xB5, 0xD8, 0xBF, 0xE9, 0xC3, 0xFB, 0xB3, 0xC6];
    let data = build_dbf("NAME", 20, &[GBK_NAME]);

    let tmp = tempfile::tempdir().expect("tmp");
    let p = tmp.path().join("gbk_cpg.dbf");
    std::fs::write(&p, &data).expect("写 DBF");
    std::fs::write(p.with_extension("cpg"), "GBK").expect("写 .cpg");

    let (names, records) = shp::read_dbf(&p).expect("读 GBK DBF（带 .cpg）");
    assert_eq!(names, vec!["NAME".to_string()]);
    assert_eq!(records[0][0], "地块名称", ".cpg 声明 GBK 时应按 GBK 解码");
}
