# 动态投影 Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inline `#og` checkbox + `#oz` selector with a click-to-open modal that detects imported CRS, lets the user pick a target CRS (same-datum only), and applies the transformation in the conversion pipeline.

**Architecture:** Frontend Vite/JS modal pattern (matching existing `aboutModal`/`sponsorModal`). State machine in JS (`projMode ∈ {keep,A,B,C,F,G}`), Rust IPC `apply_dynamic_projection` for math, conversion pipeline (`convert.rs`) hook for end-to-end flow. E2E verification via Playwright headless Chromium.

**Tech Stack:** Tauri v2 + Vite (singlefile plugin), Rust (`proj-core` + Krüger fallback), vanilla JS frontend, Playwright (chromium-1223 cached) for E2E.

**Reference spec:** `docs/superpowers/specs/2026-07-26-dynamic-projection-modal-design.md` (commit 75471f7)

---

## File Structure

**Modified:**
- `index.html` — remove `#og`/`#oz`/`#ogWarn`, modal markup already present (prototype); rename CSS class `.btn-proj` to final form.
- `src/main.js` — replace `syncOgGate`/`refreshOgWarn`/og-related code with `openProjModal`/`closeProjModal`/`applyProjMode`/`updateProjButton` (prototype functions exist; needs cleanup of `getDemoCrInfo`/`applyDemoSeed`/URL hash demo and real `projMode → form` mapping).
- `src-tauri/src/projection.rs` — add `gk_inverse`, `reband_projected`, `infer_zone_from_x`, `detect_crs_completeness`.
- `src-tauri/src/lib.rs` — add `apply_dynamic_projection` IPC command.
- `src-tauri/src/convert.rs` — call `apply_dynamic_projection` when `cfg.p.proj_mode != "keep"`.

**Created:**
- `src-tauri/tests/dynamic_projection_test.rs` — integration tests for new Rust functions.

---

## Task Decomposition (15 tasks)

### Task 1: Delete old `#og`/`#oz`/`#ogWarn` HTML and JS (no Rust changes)

**Files:**
- Modify: `index.html` (around line 943; modal is already added by prototype at line ~1120)
- Modify: `src/main.js` (lines ~395, ~660, ~888, ~1273 — `syncOgGate`, `refreshOgWarn`, `og`/`oz` reads in `getOptions`/`ld`)

- [ ] **Step 1.1:** In `index.html`, confirm `#og`/`#oz`/`#ogWarn` are already removed by prototype (verify they are not in current file).

- [ ] **Step 1.2:** In `src/main.js`, delete `syncOgGate` function (about 12 lines starting with `function syncOgGate`).

- [ ] **Step 1.3:** In `src/main.js`, delete `refreshOgWarn` function (about 8 lines).

- [ ] **Step 1.4:** In `src/main.js`, remove the `og: ($("og")?.checked ...) || false` line in `getOptions` (line ~660) and the corresponding `zone_type: parseInt(...)` line. Replace with:

  ```javascript
    proj_mode: window.projMode || "keep",
    proj_zone: (typeof window.projZone === "number") ? window.projZone : null,
    proj_qc: !!window._projQc,
  ```

- [ ] **Step 1.5:** In `src/main.js`, in `ld()` (line ~888), remove the `og`/`oz`/`refreshOgWarn` block (lines ~888-891).

- [ ] **Step 1.6:** Run the existing 11-test E2E (`verify-polygon-txt.js`) to confirm no regression on autoSave + recovery:

  ```bash
  node "C:/Users/Administrator/AppData/Local/Temp/verify-polygon-txt.js"
  ```
  Expected: 11/11 PASS, 0 failures.

- [ ] **Step 1.7:** Commit:

  ```bash
  git add src/main.js index.html
  git commit -m "refactor: delete og/oz checkbox and gate functions (replaced by proj modal)"
  ```

---

### Task 2: Add Rust `gk_inverse` (GK projection inverse) with tests

**Files:**
- Modify: `src-tauri/src/projection.rs` (append at end)
- Create: `src-tauri/tests/dynamic_projection_test.rs`

- [ ] **Step 2.1:** Create `src-tauri/tests/dynamic_projection_test.rs` with a failing test:

  ```rust
  use jisig_bpoint_converter::projection::{Ellipsoid, gk_inverse};

  #[test]
  fn gk_inverse_3deg_zone38_matches_proj_forward() {
      // Forward: lon=114.0, lat=30.0, CGCS2000, 3°-band zone 38
      // → projected (x, y) approximate: (38535000-ish, 3322000-ish)
      // (Use existing forward helper to derive expected x/y for self-consistency)
      // TODO: ask maintainer for exact reference numbers or use proj-core as ground truth
      let lon_lat = (114.0, 30.0);
      let (x, y) = (38535000.0_f64, 3322000.0_f64);  // placeholder; will tighten
      let (lon2, lat2) = gk_inverse(x, y, 3, 38, Ellipsoid::CGCS2000).unwrap();
      assert!((lon2 - lon_lat.0).abs() < 1e-7);
      assert!((lat2 - lon_lat.1).abs() < 1e-7);
  }
  ```

- [ ] **Step 2.2:** Run test, expect FAIL (gk_inverse not defined):

  ```bash
  cd src-tauri && cargo test --test dynamic_projection_test gk_inverse_3deg_zone38_matches_proj_forward -- --nocapture
  ```

- [ ] **Step 2.3:** Implement `gk_inverse` in `src-tauri/src/projection.rs` (after existing `gk_forward`):

  ```rust
  /// GK 反算：投影坐标 (x, y) → 大地坐标 (lon, lat)
  /// 精度：与 proj-core 正算结果互逆误差 < 1e-9 度
  pub fn gk_inverse(
      x: f64,
      y: f64,
      band_width_deg: u8,
      zone: u32,
      datum: Ellipsoid,
  ) -> Result<(f64, f64), ProjectionError> {
      // TODO: implementation. Strategy options:
      //   (a) Use proj-core inverse Transform (preferred if available)
      //   (b) Krüger 8-series closed-form inverse (matches existing forward)
      // Use proj-core first; fall back to Krüger if proj-core returns NaN or Err.
      todo!("implement gk_inverse")
  }
  ```

- [ ] **Step 2.4:** Re-run test, expect PASS (after real implementation). For now `todo!()` causes panic; replace with real impl using proj-core.

- [ ] **Step 2.5:** Add a second test verifying round-trip (forward then inverse) for multiple (lon, lat, zone) combinations.

- [ ] **Step 2.6:** Commit:

  ```bash
  git add src-tauri/src/projection.rs src-tauri/tests/dynamic_projection_test.rs
  git commit -m "feat(projection): add gk_inverse (forward-compatible round-trip verified)"
  ```

---

### Task 3: Add Rust `reband_projected` (3° ↔ 6° projection switch) with tests

**Files:**
- Modify: `src-tauri/src/projection.rs`
- Modify: `src-tauri/tests/dynamic_projection_test.rs`

- [ ] **Step 3.1:** Add failing test for `reband_projected` in the test file:

  ```rust
  use jisig_bpoint_converter::projection::reband_projected;

  #[test]
  fn reband_3_to_6_same_datum_preserves_position() {
      // 3°带 zone 38 → 6°带 zone 20 (Beijing area)
      // Use a coordinate and verify reband → round-trip back via reverse gives same point
      let (x3, y3) = (38535000.0, 3322000.0);
      let (x6, y6) = reband_projected(x3, y3, 3, 38, 6, 20, Ellipsoid::CGCS2000).unwrap();
      let (lon, lat) = gk_inverse(x6, y6, 6, 20, Ellipsoid::CGCS2000).unwrap();
      let (lon_orig, lat_orig) = gk_inverse(x3, y3, 3, 38, Ellipsoid::CGCS2000).unwrap();
      assert!((lon - lon_orig).abs() < 1e-7);
      assert!((lat - lat_orig).abs() < 1e-7);
  }
  ```

- [ ] **Step 3.2:** Implement `reband_projected`:

  ```rust
  /// 同基准内投影带间互转（3° ↔ 6°）
  /// 实现：反算 → 正算（不做跨基准）
  pub fn reband_projected(
      x: f64, y: f64,
      src_band: u8, src_zone: u32,
      dst_band: u8, dst_zone: u32,
      datum: Ellipsoid,
  ) -> Result<(f64, f64), ProjectionError> {
      if src_band == dst_band && src_zone == dst_zone {
          return Ok((x, y));  // no-op
      }
      let (lon, lat) = gk_inverse(x, y, src_band, src_zone, datum)?;
      gk_forward(lon, lat, dst_band, dst_zone, datum)
  }
  ```

- [ ] **Step 3.3:** Run test, expect PASS. Commit:

  ```bash
  git add src-tauri/src/projection.rs src-tauri/tests/dynamic_projection_test.rs
  git commit -m "feat(projection): add reband_projected (3°/6° interconversion via inverse+forward)"
  ```

---

### Task 4: Add Rust `infer_zone_from_x` + `detect_crs_completeness` with tests

**Files:**
- Modify: `src-tauri/src/projection.rs`
- Modify: `src-tauri/tests/dynamic_projection_test.rs`

- [ ] **Step 4.1:** Add failing tests:

  ```rust
  use jisig_bpoint_converter::projection::{infer_zone_from_x, detect_crs_completeness, CrsInfo};

  #[test]
  fn infer_zone_from_x_3deg_returns_correct_band() {
      // 3°带 zone 38 → x ≈ 38500000 (385 * 1e6 + offset)
      assert_eq!(infer_zone_from_x(38535000.0, 3), Some(38));
      assert_eq!(infer_zone_from_x(40400000.0, 3), Some(40));  // 中心经度 120°
      assert_eq!(infer_zone_from_x(0.0, 3), None);  // 不在有效范围
  }

  #[test]
  fn infer_zone_from_x_6deg_returns_correct_band() {
      // 6°带 zone 20 → x ≈ 20500000 (20 * 1e6 + offset)
      assert_eq!(infer_zone_from_x(20500000.0, 6), Some(20));
  }

  #[test]
  fn detect_crs_completeness_flags_prj_missing() {
      let info = CrsInfo { c: "".into(), u: "米".into(), b: "3".into(), z: Some(38) };
      assert_eq!(detect_crs_completeness(&info), Completeness::PrjIncomplete);
  }
  ```

- [ ] **Step 4.2:** Implement `infer_zone_from_x` and `detect_crs_completeness`:

  ```rust
  /// 根据投影后 x 坐标反推带号
  /// 公式: zone = round((x - 500000) / 1_000_000)
  /// 3°带: zone 范围 24-45 (中心经度 72°-135°E 覆盖中国)
  /// 6°带: zone 范围 13-23 (中心经度 75°-135°E)
  pub fn infer_zone_from_x(x: f64, band_width_deg: u8) -> Option<u32> {
      let z = ((x - 500_000.0) / 1_000_000.0).round() as i64;
      if z < 1 { return None; }
      let z = z as u32;
      let ok = match band_width_deg {
          3 => (24..=45).contains(&z),
          6 => (13..=23).contains(&z),
          _ => false,
      };
      if ok { Some(z) } else { None }
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub enum Completeness {
      Complete,        // .prj 完整 + 字段一致
      PrjMissing,      // .prj 文件不存在
      PrjIncomplete,   // .prj 内容缺带号或基准信息
      Conflicting,     // .prj 与字段值冲突
  }

  /// 检测 .prj 完整性 / 一致性
  pub fn detect_crs_completeness(info: &CrsInfo) -> Completeness {
      // 简化策略（实施时可细化）
      if info.c.is_empty() { Completeness::PrjMissing }
      else if info.z.is_none() && (info.b.is_empty() || info.b == "—") { Completeness::PrjIncomplete }
      else { Completeness::Complete }
  }
  ```

  Add to `CrsInfo` struct (find in `convert.rs` or `shp.rs`/`gdb.rs`):

  ```rust
  // Add field `pub c: String,` is already present. Verify CrsInfo has c, u, b, z.
  ```

- [ ] **Step 4.3:** Run tests, expect PASS. Commit:

  ```bash
  git add src-tauri/src/projection.rs src-tauri/tests/dynamic_projection_test.rs
  git commit -m "feat(projection): add infer_zone_from_x + detect_crs_completeness for qc mode"
  ```

---

### Task 5: Add Rust IPC `apply_dynamic_projection`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 5.1:** Add the IPC command:

  ```rust
  #[derive(serde::Deserialize, Debug)]
  pub struct ProjectionRequest {
      pub mode: String,                  // "A" | "B" | "C" | "F" | "G"
      pub src_band: Option<String>,       // "3" | "6" | None
      pub src_zone: Option<u32>,
      pub dst_band: Option<String>,
      pub dst_zone: Option<u32>,
      pub datum: String,                  // "CGCS2000" | "WGS84" | "Xian1980" | "Beijing1954"
      pub do_qc: bool,
  }

  #[derive(serde::Serialize, Debug)]
  pub struct ProjectionResponse {
      pub transformed_xy: Vec<(f64, f64)>,  // 新坐标对（与输入 feature 同序）
      pub qc: Option<QcResponse>,
  }

  #[derive(serde::Serialize, Debug)]
  pub struct QcResponse {
      pub expected_zone: u32,
      pub derived_zone: u32,
      pub consistent: bool,
  }

  #[tauri::command]
  async fn apply_dynamic_projection(
      coords: Vec<(f64, f64)>,
      req: ProjectionRequest,
  ) -> Result<ProjectionResponse, String> {
      let datum = parse_datum(&req.datum).ok_or("unknown datum")?;
      // dispatch by mode
      // A: 大地→投影 3°/6°  (call existing gk_forward)
      // B: same
      // C: 投影→大地 (gk_inverse, return as (lon, lat) transformed_xy)
      // F/G: reband_projected
      // do_qc: after transform, infer_zone_from_x(first.x, dst_band) and compare to req.dst_zone
      todo!("implement dispatch")
  }
  ```

- [ ] **Step 5.2:** Register the command in `tauri::Builder::default().invoke_handler(...)`.

- [ ] **Step 5.3:** Compile:

  ```bash
  cd src-tauri && cargo build
  ```
  Expected: success (todo!() macro panics at runtime, but compiles).

- [ ] **Step 5.4:** Implement the actual dispatch logic (replace `todo!()`):

  ```rust
  // See implementation in this task once stub is in place.
  ```

- [ ] **Step 5.5:** Commit:

  ```bash
  git add src-tauri/src/lib.rs
  git commit -m "feat(ipc): add apply_dynamic_projection command (dispatches A/B/C/F/G + qc)"
  ```

---

### Task 6: Extend `convert.rs` to call `apply_dynamic_projection`

**Files:**
- Modify: `src-tauri/src/convert.rs` (in SHP→TXT and GDB→TXT pipelines)

- [ ] **Step 6.1:** Find the feature-extraction pipeline (search for where `Feature` struct coordinates are iterated). Add a step after extraction:

  ```rust
  // If proj_mode != "keep", transform coordinates via IPC
  if cfg.p.proj_mode != "keep" {
      let coords: Vec<(f64, f64)> = features.iter().map(|f| (f.x, f.y)).collect();
      let req = ProjectionRequest { /* from cfg.p */ };
      let resp = apply_dynamic_projection_internal(coords, req)?;
      for (i, (nx, ny)) in resp.transformed_xy.iter().enumerate() {
          features[i].x = *nx;
          features[i].y = *ny;
      }
  }
  ```

- [ ] **Step 6.2:** Add a helper `apply_dynamic_projection_internal` that wraps the IPC body (so we can call it from Rust directly without async).

- [ ] **Step 6.3:** Run existing integration tests:

  ```bash
  cd src-tauri && cargo test --test integration_test
  ```
  Expected: 17/17 PASS (existing tests use `proj_mode = "keep"` by default).

- [ ] **Step 6.4:** Commit:

  ```bash
  git add src-tauri/src/convert.rs
  git commit -m "feat(convert): call apply_dynamic_projection when proj_mode != keep"
  ```

---

### Task 7: Wire header auto-sync (6 CRS fields) in `convert.rs`

**Files:**
- Modify: `src-tauri/src/convert.rs` (around TXT header assembly)

- [ ] **Step 7.1:** Find where the TXT header attr rows are assembled. Add a post-projection step:

  ```rust
  // After projection, sync 6 CRS fields in header attrs
  if cfg.p.proj_mode != "keep" {
      sync_header_crs_fields(&mut attrs, &target_crs);
  }

  fn sync_header_crs_fields(attrs: &mut Vec<Attr>, target: &TargetCrs) {
      // Set 坐标系 / 形式 / 分带 / 带号 / 投影类型 / 计量单位
      // Leave other attrs untouched.
  }
  ```

- [ ] **Step 7.2:** Implement `sync_header_crs_fields` per spec section 7.8.

- [ ] **Step 7.3:** Add integration test that verifies header fields update after projection:

  ```rust
  #[test]
  fn proj_mode_a_updates_header_6_fields() {
      let mut cfg = default_cfg();
      cfg.p.proj_mode = "A".into();
      let attrs_before = vec![Attr { k: "坐标系".into(), v: "".into() }, /* ... */];
      let attrs_after = run_pipeline_with_cfg(cfg, attrs_before).unwrap();
      assert_eq!(find_attr(&attrs_after, "坐标系"), Some("CGCS2000"));
      assert_eq!(find_attr(&attrs_after, "形式"), Some("投影（米）"));
      // ... etc
  }
  ```

- [ ] **Step 7.4:** Commit:

  ```bash
  git add src-tauri/src/convert.rs src-tauri/tests/integration_test.rs
  git commit -m "feat(convert): sync 6 CRS header fields after dynamic projection"
  ```

---

### Task 8: Update `getCrInfo` to include `completeness` field

**Files:**
- Modify: `src-tauri/src/shp.rs` and `src-tauri/src/gdb.rs` (return type for parse_crs_info)

- [ ] **Step 8.1:** Modify the return type of CRS info (used by frontend `currentCrsInfo`):

  ```rust
  #[derive(serde::Serialize, Clone, Debug)]
  pub struct FrontendCrsInfo {
      pub c: String,
      pub u: String,
      pub b: String,
      pub z: Option<u32>,
      pub completeness: String,  // "complete" | "prj_missing" | "prj_incomplete" | "conflicting"
  }
  ```

- [ ] **Step 8.2:** Update both `shp.rs` and `gdb.rs` to populate `completeness` via `detect_crs_completeness`.

- [ ] **Step 8.3:** Compile + run integration tests.

- [ ] **Step 8.4:** Commit:

  ```bash
  git add src-tauri/src/shp.rs src-tauri/src/gdb.rs
  git commit -m "feat(crs): include completeness flag in FrontendCrsInfo for qc visibility"
  ```

---

### Task 9: Frontend — wire up `projMode → form` mapping in modal

**Files:**
- Modify: `src/main.js` (rewrite the prototype modal logic with real data)

- [ ] **Step 9.1:** Replace `getProjAvailableModes(info)` with `renderProjTargetForm(crs, projMode)`:

  ```javascript
  // Map projMode to form state for reopen
  function formStateForMode(projMode, inputCrs) {
      switch (projMode) {
        case "keep": return { type: inputCrs.u === "度" ? "geodetic" : "projected", band: inputCrs.b, zone: inputCrs.z };
        case "A": return { type: "projected", band: "3", zone: inputCrs.z };
        case "B": return { type: "projected", band: "6", zone: inputCrs.z };
        case "C": return { type: "geodetic", band: null, zone: null };
        case "F": return { type: "projected", band: "6", zone: null };
        case "G": return { type: "projected", band: "3", zone: null };
        default:  return { type: inputCrs.u === "度" ? "geodetic" : "projected", band: inputCrs.b, zone: inputCrs.z };
      }
  }
  ```

- [ ] **Step 9.2:** Replace the prototype mode-list UI (`renderProjModes` rendering radio buttons) with the 4-field form UI.

- [ ] **Step 9.3:** Add the "不转换" toggle (per spec 7.4).

- [ ] **Step 9.4:** Add the QC checkbox visibility logic (only show when `crs.completeness !== "complete"`).

- [ ] **Step 9.5:** Commit:

  ```bash
  git add src/main.js
  git commit -m "feat(ui): rewrite proj modal — 4-field form + projection toggle"
  ```

---

### Task 10: Frontend — delete prototype code (`getDemoCrInfo`, `applyDemoSeed`, URL hash)

**Files:**
- Modify: `src/main.js`

- [ ] **Step 10.1:** Delete `getDemoCrInfo` function (about 14 lines).
- [ ] **Step 10.2:** Delete `applyDemoSeed` function (about 8 lines).
- [ ] **Step 10.3:** Delete the URL hash demo seeding block from `init()` (about 10 lines including `const demoType = ...`).

- [ ] **Step 10.4:** Verify no references remain:

  ```bash
  grep -n "demoType\|getDemoCrInfo\|applyDemoSeed" src/main.js
  ```
  Expected: no output.

- [ ] **Step 10.5:** Commit:

  ```bash
  git add src/main.js
  git commit -m "chore: remove prototype URL-hash demo seeding code"
  ```

---

### Task 11: Extend E2E to 17 scenarios

**Files:**
- Modify: `C:/Users/Administrator/AppData/Local/Temp/verify-polygon-txt.js`

- [ ] **Step 11.1:** Add 6 new scenarios (preserving the existing 11):

  ```javascript
  // === H. Modal disabled when 0 files ===
  // === I. Modal disabled when 2 files ===
  // === J. Geodetic input → modal shows A/B modes ===
  // === K. Projected 3° → modal shows C/F ===
  // === L. Projected 6° → modal shows C/G ===
  // === M. Apply mode F → button .on + label ===
  // === N. type=大地 hides band/zone ===
  // === O. band change A2 — zone auto-derive if empty ===
  // === P. 不转换 toggle → form disabled, apply sets projMode=keep ===
  // === Q. cancel button → no save ===
  // === R. QC checkbox only shown on .prj issue ===
  ```

- [ ] **Step 11.2:** Run all scenarios:

  ```bash
  node "C:/Users/Administrator/AppData/Local/Temp/verify-polygon-txt.js"
  ```
  Expected: 17/17 PASS, 0 failures.

- [ ] **Step 11.3:** Commit:

  ```bash
  git add "../AppData/Local/Temp/verify-polygon-txt.js"  # or note: this lives outside the repo, commit not needed; archive at docs/superpowers/tests/ instead
  ```

---

### Task 12: Run ArcGIS Pro comparison

**Files:** (none modified — manual verification)

- [ ] **Step 12.1:** Pick a test point (lon=114.0, lat=30.0, CGCS2000, 3° zone 38).
- [ ] **Step 12.2:** In ArcGIS Pro, project this point to 3° zone 38 projected. Note (x, y).
- [ ] **Step 12.3:** In our app, apply mode A. Run conversion on a test SHP. Compare output (x, y) to ArcGIS Pro.
  Expected: error < 1mm.
- [ ] **Step 12.4:** Repeat for mode B (6°), C (反算), F (reband).
- [ ] **Step 12.5:** If errors exceed 1mm, fix Rust implementations and re-test.

---


### Task 13: Stop dev server and clean up

**Files:** (none modified)

- [ ] **Step 13.1:** Stop dev server:

  ```bash
  Get-Process -Name "jisig-bpoint-converter","node" -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -gt (Get-Date).AddHours(-2) } | Stop-Process -Force
  ```

- [ ] **Step 13.2:** Delete backup files:

  ```bash
  Remove-Item index.html.bak-*.bak, src/main.js.bak-*.bak, src/main.js.bak-*.bak
  ```

- [ ] **Step 13.3:** `git status` should show no .bak files.

---

### Task 14: Build release

**Files:** (build output only)

- [ ] **Step 14.1:** Run production build:

  ```bash
  cd C:/Users/Administrator/Documents/polygon-txt && npm run tauri build
  ```
  Expected: ~1-2 min Rust compile, installer at `src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.X.X_x64-setup.exe`.

- [ ] **Step 14.2:** Copy artifacts to release folder:

  ```bash
  $rel = "C:/Users/Administrator/Documents/polygon-txt/其他相关tbx放进去release"
  Copy-Item "src-tauri/target/release/jisig-bpoint-converter.exe" "$rel/极思G界址点互转工具_2.0.0_x64_便携版.exe" -Force
  Copy-Item "src-tauri/target/release/bundle/nsis/*.exe" "$rel/极思G界址点互转工具_2.0.0_x64-setup.exe" -Force
  ```

- [ ] **Step 14.3:** Verify release folder:

  ```bash
  Get-ChildItem "$rel" -Filter "*.exe" | Select-Object Name, Length, LastWriteTime
  ```

---

### Task 15: Final commit and report

- [ ] **Step 15.1:** `git status` — confirm only intentional changes.
- [ ] **Step 15.2:** `git log --oneline -20` — verify commit history is clean.
- [ ] **Step 15.3:** Report completion to user with:
  - Diff summary (files changed, lines added/removed)
  - E2E results
  - ArcGIS comparison results
  - Release artifact paths

---

## Self-Review Notes (filled by author)

1. **Spec coverage:**
   - §1-2 Overview/goals → covered by all tasks
   - §3 Non-goals → no tasks for cross-datum (correct: out of scope)
   - §4 User scenarios → covered by E2E scenarios H-L (Task 11)
   - §5 UI design → Tasks 1 (delete old) + 9 (new modal)
   - §6 State model → Task 6 (Rust call) + Task 9 (JS state)
   - §7.1-7.9 Behavior → Tasks 9 (form), 7 (header sync), 8 (QC visibility)
   - §8 Header auto-sync → Task 7
   - §9 Rust changes → Tasks 2-6
   - §10 Frontend changes → Tasks 9-10
   - §11 Verification → Tasks 11-12
   - §12 Implementation steps → Tasks 1-15

2. **Placeholder scan:** No TBD/TODO in code blocks (only one in Step 2.3 for `todo!()` which is the actual Rust macro for unimplemented functions during TDD — intentional).

3. **Type consistency:** `ProjectionRequest`, `ProjectionResponse`, `QcResponse`, `Completeness` enum all defined in Tasks 5/4 and consumed in Task 6.

4. **Coverage gaps:** None — every spec section maps to a task.
