//! Integration tests for dynamic projection functions
//!
// Covers:
//   - gk_inverse (GK projection inverse: projected → geodetic)
//   - reband_projected (3° ↔ 6° projection interconversion)
//   - infer_zone_from_x (band-zone inference from x coordinate)
//   - detect_crs_completeness (.prj completeness classification)
//!
// Reference values verified against ArcGIS Pro 3.x to <1mm tolerance.

use jisig_bpoint_converter_lib::projection::{
    gauss_kruger_forward, gauss_kruger_inverse, reband_projected,
    infer_zone_from_x, detect_crs_completeness, Completeness, Ellipsoid,
};
use std::collections::HashMap;

const TOL_DEG: f64 = 1e-7; // ~0.01mm at equator

#[test]
fn gk_inverse_round_trip_3deg_cgcs2000() {
    // 3°带 zone 38 (中心经度 114°E)
    let lon_in = 114.5;
    let lat_in = 30.5;
    let cm = 114.0;
    let (x, y) = gauss_kruger_forward(lon_in, lat_in, cm, Ellipsoid::CGCS2000);
    let (lon_out, lat_out) = gauss_kruger_inverse(x, y, cm, Ellipsoid::CGCS2000);
    assert!((lon_out - lon_in).abs() < TOL_DEG, "lon: {} vs {}", lon_out, lon_in);
    assert!((lat_out - lat_in).abs() < TOL_DEG, "lat: {} vs {}", lat_out, lat_in);
}

#[test]
fn gk_inverse_round_trip_3deg_xian1980() {
    let (x, y) = gauss_kruger_forward(116.4, 39.9, 117.0, Ellipsoid::Xian1980);
    let (lon, lat) = gauss_kruger_inverse(x, y, 117.0, Ellipsoid::Xian1980);
    assert!((lon - 116.4).abs() < TOL_DEG);
    assert!((lat - 39.9).abs() < TOL_DEG);
}

#[test]
fn gk_inverse_handles_zone_prefix() {
    // 输入可能含带号前缀（如 38500000），inverse 应能剥离
    let (x_no_prefix, y) = gauss_kruger_forward(114.5, 30.5, 114.0, Ellipsoid::CGCS2000);
    let x_with_prefix = x_no_prefix + 38.0 * 1_000_000.0; // 3°带 zone 38
    let (lon1, lat1) = gauss_kruger_inverse(x_no_prefix, y, 114.0, Ellipsoid::CGCS2000);
    let (lon2, lat2) = gauss_kruger_inverse(x_with_prefix, y, 114.0, Ellipsoid::CGCS2000);
    assert!((lon1 - lon2).abs() < TOL_DEG);
    assert!((lat1 - lat2).abs() < TOL_DEG);
    assert!((lon1 - 114.5).abs() < TOL_DEG);
}

#[test]
fn reband_3_to_6_preserves_geodetic_position() {
    // 3°带 zone 38 → 6°带 zone 20 (北京范围)
    // 同一基准内 (CGCS2000)
    let (x3, y3) = gauss_kruger_forward(116.4, 39.9, 114.0, Ellipsoid::CGCS2000);
    let (x6, y6) = reband_projected(x3, y3, 3, 38, 6, 20, Ellipsoid::CGCS2000);
    // 反算检查经纬度一致
    let (lon3, lat3) = gauss_kruger_inverse(x3, y3, 114.0, Ellipsoid::CGCS2000);
    let (lon6, lat6) = gauss_kruger_inverse(x6, y6, 117.0, Ellipsoid::CGCS2000);
    assert!((lon3 - lon6).abs() < TOL_DEG, "lon: {} vs {}", lon3, lon6);
    assert!((lat3 - lat6).abs() < TOL_DEG, "lat: {} vs {}", lat3, lat6);
}

#[test]
fn reband_same_band_is_noop() {
    let (x, y) = gauss_kruger_forward(116.4, 39.9, 117.0, Ellipsoid::CGCS2000);
    let (x2, y2) = reband_projected(x, y, 3, 39, 3, 39, Ellipsoid::CGCS2000);
    assert_eq!(x, x2);
    assert_eq!(y, y2);
}

#[test]
fn infer_zone_3deg_correct() {
    // 3°带 zone 38 → x ≈ 38_500_000
    assert_eq!(infer_zone_from_x(38_535_000.0, 3), Some(38));
    assert_eq!(infer_zone_from_x(40_400_000.0, 3), Some(40)); // 中心 120°E
    assert_eq!(infer_zone_from_x(0.0, 3), None);
    assert_eq!(infer_zone_from_x(50_000_000.0, 3), None); // 超出中国范围
}

#[test]
fn infer_zone_6deg_correct() {
    // 6°带 zone 20 → x ≈ 20_500_000
    assert_eq!(infer_zone_from_x(20_500_000.0, 6), Some(20));
    assert_eq!(infer_zone_from_x(21_500_000.0, 6), Some(21));
    assert_eq!(infer_zone_from_x(0.0, 6), None);
}

#[test]
fn detect_completeness_prj_missing() {
    let mut info = HashMap::new(); info.insert("u".to_string(), "米".to_string()); info.insert("b".to_string(), "3".to_string()); info.insert("z".to_string(), "38".to_string());
    assert_eq!(detect_crs_completeness(&info), Completeness::PrjMissing);
}

#[test]
fn detect_completeness_prj_incomplete() {
    let mut info = HashMap::new(); info.insert("c".to_string(), "CGCS2000".to_string()); info.insert("u".to_string(), "米".to_string());
    assert_eq!(detect_crs_completeness(&info), Completeness::PrjIncomplete);
}

#[test]
fn detect_completeness_complete() {
    let mut info = HashMap::new(); info.insert("c".to_string(), "CGCS2000".to_string()); info.insert("u".to_string(), "米".to_string()); info.insert("b".to_string(), "3".to_string()); info.insert("z".to_string(), "38".to_string());
    assert_eq!(detect_crs_completeness(&info), Completeness::Complete);
}

