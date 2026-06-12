# -*- coding: utf-8 -*-
"""ArcPy verification for generated GeoPackage output."""

import os
import sys

import arcpy

TEMP_DIR = os.environ.get("TEMP", os.environ.get("TMP", "."))
GPKG_DIR = os.environ.get("JISIG_GPKG_DIR", os.path.join(TEMP_DIR, "jisig_arcpy_verify"))
GPKG_PATH = None


def check(label, ok, detail=""):
    suffix = f"  ({detail})" if detail else ""
    print(f"[{'PASS' if ok else 'FAIL'}] {label}{suffix}")
    return ok


def main():
    global GPKG_PATH
    if os.path.isdir(GPKG_DIR):
        gpkg_files = sorted(
            os.path.join(GPKG_DIR, name)
            for name in os.listdir(GPKG_DIR)
            if name.lower().endswith(".gpkg")
        )
        if gpkg_files:
            GPKG_PATH = gpkg_files[0]

    print("ArcPy GeoPackage verification")
    print(f"ArcPy version: {arcpy.GetInstallInfo()['Version']}")
    print(f"GPKG path: {GPKG_PATH}")

    ok = True
    ok &= check("GPKG exists", GPKG_PATH is not None and os.path.exists(GPKG_PATH))
    if not ok:
        return 1

    try:
        arcpy.env.workspace = GPKG_PATH
        fcs = arcpy.ListFeatureClasses()
        ok &= check("ListFeatureClasses returns feature classes", len(fcs) > 0, fcs)
        for fc in fcs:
            desc = arcpy.Describe(fc)
            ok &= check(f"{fc} shapeType is Polygon", desc.shapeType == "Polygon", desc.shapeType)
            sr = getattr(desc, "spatialReference", None)
            ok &= check(f"{fc} spatial reference exists", sr is not None)
            if sr:
                ok &= check(
                    f"{fc} CRS name looks projected",
                    "CGCS2000" in sr.name or "4526" in sr.name or "4547" in sr.name,
                    sr.name,
                )
            count = int(arcpy.management.GetCount(fc)[0])
            ok &= check(f"{fc} has at least 1 feature", count > 0, count)
            with arcpy.da.SearchCursor(fc, ["SHAPE@"]) as cursor:
                first = next(cursor, None)
                ok &= check(f"{fc} geometry is present", first is not None and first[0] is not None)
    except Exception as exc:
        print(f"[FAIL] Read error: {exc}")
        ok = False
    finally:
        arcpy.env.workspace = None

    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
