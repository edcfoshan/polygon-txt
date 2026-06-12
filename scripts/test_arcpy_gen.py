"""
界址点互转工具 — ArcPy 测试数据生成
生成标准 SHP 和 GDB 测试数据，用于验证 Rust 转换工具
"""
import arcpy, os, json, shutil
from pathlib import Path

WORK = Path(r"C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy")
TXT_SRC = Path(r"D:\00结束\本地肇庆高新区数据治理\05开始录入\2所有都是0错误\肇庆高新区txt")

if WORK.exists():
    shutil.rmtree(str(WORK))
WORK.mkdir(parents=True)
arcpy.env.overwriteOutput = True

def parse_txt_coords(txt_path):
    """解析 TXT 文件，返回地块列表"""
    with open(txt_path, "r", encoding="utf-8") as f:
        text = f.read()
    
    plots = []
    section = ""
    current = None
    for line in text.split("\n"):
        line = line.strip()
        if line == "[地块坐标]":
            section = "coord"
            continue
        elif line.startswith("["):
            section = ""
            continue
        elif section != "coord":
            continue
        
        if ",@" in line or line.endswith("@"):
            parts = line.split(",")
            current = {"name": parts[3] if len(parts) > 3 else "", "coords": []}
            plots.append(current)
        elif current and "," in line:
            parts = line.split(",")
            try:
                y = float(parts[-2] if len(parts) >= 4 else parts[-2])
                x = float(parts[-1])
                current["coords"].append((x, y))  # SHP format: (X, Y)
            except:
                pass
    return plots

# ─── 1. 从 TXT 创建标准 SHP ───
print("=== 从 TXT 创建标准 SHP ===")
std_shp_dir = WORK / "std_shp"
std_shp_dir.mkdir()

txt_files = sorted(TXT_SRC.glob("*.txt"))[:5]
sr = arcpy.SpatialReference(4490)  # CGCS2000

for i, txt_path in enumerate(txt_files):
    plots = parse_txt_coords(txt_path)
    if not plots:
        continue
    
    out_shp = str(std_shp_dir / f"plot_{i:03d}.shp")
    arcpy.CreateFeatureclass_management(str(std_shp_dir), f"plot_{i:03d}.shp", "POLYGON", spatial_reference=sr)
    arcpy.AddField_management(out_shp, "DKMC", "TEXT", field_length=50)
    arcpy.AddField_management(out_shp, "DKBH", "TEXT", field_length=30)
    arcpy.AddField_management(out_shp, "MJ", "DOUBLE")
    arcpy.AddField_management(out_shp, "DKYT", "TEXT", field_length=50)
    arcpy.AddField_management(out_shp, "TFH", "TEXT", field_length=20)
    arcpy.AddField_management(out_shp, "DLBM", "TEXT", field_length=10)
    
    with arcpy.da.InsertCursor(out_shp, ["SHAPE@", "DKMC", "DKBH", "MJ", "DKYT", "TFH", "DLBM"]) as cur:
        for plot in plots:
            if len(plot["coords"]) >= 3:
                array = arcpy.Array([arcpy.Point(*pt) for pt in plot["coords"]])
                polygon = arcpy.Polygon(array, sr)
                cur.insertRow([polygon, plot["name"], "", 0, "", "", ""])
    
    count = int(arcpy.GetCount_management(out_shp).getOutput(0))
    print(f"  {txt_path.name} -> {out_shp} ({count} 要素)")

print(f"\n生成 {len(list(std_shp_dir.glob('*.shp')))} 个标准 SHP 文件")

# ─── 2. 创建测试 GDB ───
print("\n=== 创建测试 GDB ===")
test_gdb = str(WORK / "test.gdb")
arcpy.CreateFileGDB_management(str(WORK), "test.gdb")

# 从标准 SHP 导入到 GDB
for shp in std_shp_dir.glob("*.shp"):
    fc_name = shp.stem
    arcpy.conversion.FeatureClassToFeatureClass(str(shp), test_gdb, fc_name)
    count = int(arcpy.GetCount_management(f"{test_gdb}/{fc_name}").getOutput(0))
    print(f"  SHP {shp.name} -> GDB/{fc_name} ({count} 要素)")

# ─── 3. 从标准 SHP 导出 TXT ───
print("\n=== 从标准 SHP 导出 TXT ===")
txt_out = WORK / "txt_output"
txt_out.mkdir()

for shp in std_shp_dir.glob("*.shp"):
    fc_name = shp.stem
    out_txt = txt_out / shp.with_suffix(".txt").name
    
    txt_lines = ["[属性描述]", "坐标系=2000国家大地坐标系", "几度分带=3", 
                 "投影类型=高斯克吕格", "计量单位=米", "带号=38",
                 "精度=0.001", "转换参数=,,,,,,", "[地块坐标]"]
    
    with arcpy.da.SearchCursor(str(shp), ["SHAPE@", "DKMC", "DKBH", "MJ", "DKYT", "TFH", "DLBM"]) as cur:
        for row in cur:
            geom = row[0]
            dkmc = row[1] or ""
            dkbh = row[2] or ""
            mj = str(row[3]) if row[3] else ""
            dkyt = row[4] or ""
            tfh = row[5] or ""
            dlbm = row[6] or ""
            
            if geom.type == "polygon":
                ring = geom[0]
                count = len(ring)
                txt_lines.append(f"{count},{mj},FID_0,{dkmc},面,{tfh},{dkyt},{dlbm},@")
                
                for j, pt in enumerate(ring):
                    # TXT format: (Y, X) = (northing, easting)
                    txt_lines.append(f"J{j+1},1,{pt.Y:.3f},{pt.X:.3f}")
    
    with open(out_txt, "w", encoding="utf-8") as f:
        f.write("\n".join(txt_lines))
    print(f"  {shp.name} -> {out_txt.name}")
    
    # 验证与原始 TXT 的地块数一致
    orig_txt = TXT_SRC / shp.with_suffix(".txt").name
    if orig_txt.exists():
        orig_plots = [l for l in open(orig_txt, encoding="utf-8").read().split("\n") if ",@" in l]
        gen_plots = [l for l in "\n".join(txt_lines).split("\n") if ",@" in l]
        print(f"    原始: {len(orig_plots)} 地块, 导出: {len(gen_plots)} 地块")

# ─── 验证 ───
print("\n" + "=" * 50)
print("生成完毕")
print("=" * 50)
for d in ["std_shp", "test.gdb", "txt_output"]:
    p = WORK / d
    if p.exists():
        items = len(list(p.rglob("*"))) if p.is_dir() else 1
        print(f"  {d}: {items} 项")

print(f"\n工作目录: {WORK}")
