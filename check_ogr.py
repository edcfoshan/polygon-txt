from osgeo import ogr
import struct

shp_path = r'C:\Users\Administrator\AppData\Local\Temp\jisig_debug_test\debug_output.shp'
print(f"Opening: {shp_path}")
ds = ogr.Open(shp_path)
if ds is None:
    print("FAILED: ogr could not open the SHP!")
else:
    lyr = ds.GetLayer(0)
    print(f"Layer: {lyr.GetName()}")
    print(f"Features: {lyr.GetFeatureCount()}")
    feat = lyr.GetNextFeature()
    if feat:
        geom = feat.GetGeometryRef()
        print(f"Geometry: {geom.GetGeometryName()}")
        print(f"Area: {geom.GetArea()}")
        ring = geom.GetGeometryRef(0)
        print(f"Ring points: {ring.GetPointCount()}")
        for i in range(min(ring.GetPointCount(), 5)):
            x, y, _ = ring.GetPoint(i)
            print(f"  [{i}] x={x:.3f}, y={y:.3f}")
        srs = lyr.GetSpatialRef()
        if srs:
            print(f"SRS: {srs.GetName()}")
            print(f"Is projected: {srs.IsProjected()}")
            if srs.IsProjected():
                print(f"Linear units: {srs.GetLinearUnitsName()}")
                print(f"Proj method: {srs.GetAttrValue('PROJECTION')}")
                print(f"False easting: {srs.GetProjParm('false_easting')}")
                print(f"Central meridian: {srs.GetProjParm('central_meridian')}")
        else:
            print("SRS: NONE")
        
        # Check fields
        ldefn = lyr.GetLayerDefn()
        print(f"\nFields:")
        for i in range(ldefn.GetFieldCount()):
            fd = ldefn.GetFieldDefn(i)
            print(f"  {fd.GetName()}: type={fd.GetTypeName()}, width={fd.GetWidth()}")
