import sys
from fontTools import subset
from fontTools.ttLib import TTFont
src, dst = sys.argv[1], sys.argv[2]
opts = subset.Options()
opts.layout_features = ['*']
opts.name_IDs = ['*']
opts.notdef_outline = True
opts.hinting = False
f = TTFont(src)
unis = list(range(0x20,0x7F)) + list(range(0xA0,0x180)) + list(range(0x2000,0x2070)) \
  + list(range(0x20A0,0x20C0)) + list(range(0x2100,0x2200)) + list(range(0x2460,0x2500)) \
  + list(range(0x3000,0x3040)) + list(range(0x3130,0x3190)) + list(range(0x3200,0x3300)) \
  + list(range(0xAC00,0xD7A4)) + list(range(0xFF00,0xFFF0)) + list(range(0x1100,0x1200))
s = subset.Subsetter(opts); s.populate(unicodes=unis); s.subset(f)
# OFL Reserved Font Name 'Nanum' 때문에 수정본은 다른 이름을 써야 한다
new = "DashboardKR"
for rec in f['name'].names:
    if rec.nameID in (1,4,16,18):
        rec.string = new + ("" if rec.nameID in (1,16) else " Regular")
    elif rec.nameID == 6:
        rec.string = new + "-Regular"
    elif rec.nameID == 3:
        rec.string = new + "-Regular;subset"
f.save(dst)
