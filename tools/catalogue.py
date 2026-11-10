#!/usr/bin/env python3
"""Material catalogue for the v1 planet types.

Writes data/materials-catalogue.toml from the table below. Each entry gives a
composition as a mix of mineral formulas (by mass), and the script turns that
into element mass fractions from data/elements.toml, so every composition sums
to exactly 1 and stays true to the chemistry.

Run: python3 tools/catalogue.py
"""
import pathlib, re, tomllib

root = pathlib.Path(__file__).resolve().parent.parent
elements = {e["symbol"]: e["mass"] for e in tomllib.load(open(root / "data/elements.toml", "rb"))["element"]}


def formula(f):
    """Element mass fractions of a formula such as 'Mg2SiO4' or 'CaMg(CO3)2'."""
    def parse(s, i=0):
        counts = {}
        while i < len(s):
            if s[i] == "(":
                inner, i = parse(s, i + 1)
                m = re.match(r"\d*\.?\d*", s[i:]).group()
                i += len(m)
                k = float(m) if m else 1.0
                for e, n in inner.items():
                    counts[e] = counts.get(e, 0) + n * k
            elif s[i] == ")":
                return counts, i + 1
            else:
                m = re.match(r"([A-Z][a-z]?)(\d*\.?\d*)", s[i:])
                e, n = m.group(1), float(m.group(2)) if m.group(2) else 1.0
                assert e in elements, f"{e} is not in elements.toml ({f})"
                counts[e] = counts.get(e, 0) + n
                i += len(m.group())
        return counts, i
    counts, _ = parse(f)
    total = sum(elements[e] * n for e, n in counts.items())
    return {e: elements[e] * n / total for e, n in counts.items()}


def mix(parts):
    """Mass-weighted mix of formulas: [(weight, 'formula'), ...]."""
    w = sum(p[0] for p in parts)
    out = {}
    for k, f in parts:
        for e, x in formula(f).items():
            out[e] = out.get(e, 0) + x * k / w
    return out


def fmt_comp(c):
    # Keep traces down to 0.1 ppm (rounded to 3 significant figures), majors
    # to 4 places, and put the rounding error on the largest element so the
    # sum is exactly 1.
    def rnd(x):
        return round(x, 6) if x >= 0.001 else float(f"{x:.3g}")
    c = {e: rnd(x) for e, x in c.items() if x >= 1e-7}
    big = max(c, key=c.get)
    c[big] = round(c[big] + 1 - sum(c.values()), 9)
    order = sorted(c, key=lambda e: -c[e])
    return "{ " + ", ".join(f"{e} = {fmt_num(c[e])}" for e in order) + " }"


def fmt_num(x):
    return f"{x:.9f}".rstrip("0").rstrip(".") if x >= 0.001 else f"{x:.3g}"


# Trace elements of common rocks (ppm by mass), after the usual averages
# (Turekian & Wedepohl 1961 for sediments and igneous rocks; Rudnick & Gao
# 2003 for the crust; Lodders 2003 for CI chondrites; McDonough 2003 for the
# core). Majors come from the mineral formulas; these are the elements no
# common mineral formula carries.
# Two trace elements per rock family at most (ppm by mass): the ones it is
# known for. Minor elements (0.1% and up) don't count against that. Anything
# more exotic comes from a body's specialties (data/specialties.toml), so a
# chunk of rock never carries a dozen traces.
TRACE = {
    "felsic": dict(Rb=150, Ba=600),
    "mafic": dict(Mn=1300, P=1000, V=250, Ga=18),
    "ultramafic": dict(Cr=2500, Ni=2000, Mn=1000, Co=110, V=60),
    "shale": dict(K=26600, Mg=15000, Ca=22100, Na=9600, Ti=4600, Mn=1000, V=130, Li=66),
    "sandstone": dict(Ti=1500, Ca=39100, Mg=7000, Na=3300, Zr=220, Ba=200),
    "carbonate": dict(Mg=47000, Fe=3800, Si=24000, Al=4200, K=2700, Mn=1100, Sr=610, F=330),
    "evaporite": dict(Br=70, B=20),
    "brine": dict(Br=200, I=0.2),
    "chondrite": dict(Ni=11000, Cr=2650, Mn=1930, P=1000, Na=5000, Co=500, Ir=0.5),
    "iron": dict(Co=5000, P=2000, Ge=40, Ir=3),
    "core": dict(Cr=9000, Co=2500, P=2000, Pt=5.7, Os=2.8),
    "ice": dict(Cl=20, Na=10),
}
TRACE_OF = {
    "felsic": ["granite", "obsidian", "pumice", "tuff", "gneiss", "quartzite", "volcanic ash", "impact breccia"],
    "mafic": ["andesite", "gabbro", "dolerite", "anorthosite", "eclogite", "red dust", "gravel"],
    "ultramafic": ["harzburgite", "pyroxenite", "plume rock", "lherzolite", "dunite", "serpentinite", "majorite",
                   "ferropericlase", "perovskite rock", "ringwoodite rock", "deep pile rock"],
    "shale": ["shale", "slate", "schist", "clay", "soil", "coal"],
    "sandstone": ["sandstone", "sand", "conglomerate", "aquifer sandstone"],
    "carbonate": ["limestone", "marble", "gypsum"],
    "evaporite": ["rock salt"],
    "brine": ["brine", "sea ice"],
    "chondrite": ["comet dust", "metal grit"],
    "iron": ["kamacite", "taenite", "plessite", "iron crystal"],
    "core": ["sulfur-rich core melt"],
    "ice": ["dirty ice", "muddy water"],
}


def with_traces(name, comp):
    """Add the rock's trace elements (ppm), keeping the composition's sum at 1."""
    prof = next((k for k, names in TRACE_OF.items() if name in names), None)
    if prof is None:
        return comp
    add = {e: v * 1e-6 for e, v in TRACE[prof].items() if v > 0 and comp.get(e, 0.0) < v * 1e-6}
    extra = sum(add[e] - comp.get(e, 0.0) for e in add)
    out = {e: x * (1 - extra) for e, x in comp.items()}
    for e, x in add.items():
        out[e] = x
    return out


# Mineral shorthands.
QTZ, FSP_K, FSP_NA, FSP_CA = "SiO2", "KAlSi3O8", "NaAlSi3O8", "CaAl2Si2O8"
OLV, PYX, MICA, CLAY = "Mg2SiO4", "MgSiO3", "KAl3Si3O10(OH)2", "Al2Si2O5(OH)4"
CALC, DOLO, MAG, HEM = "CaCO3", "CaMg(CO3)2", "Fe3O4", "Fe2O3"

# name, family, state, density, hardness, colour, multiplier, composition parts,
# extra keys (melts/boils/phase targets/flow/conduct/latent).
CAT = [
    # ---------------- igneous rocks ----------------
    ("granite", "igneous", "solid", 2700, 6, [168, 150, 140], 1.0, [(35, QTZ), (35, FSP_K), (20, FSP_NA), (10, MICA)], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("andesite", "igneous", "solid", 2650, 5, [110, 108, 104], 1.0, [(55, FSP_NA), (25, PYX), (10, QTZ), (10, MAG)], dict(melts_at_k=1450, boils_at_k=3000, latent_boil_k=5000)),
    ("gabbro", "igneous", "solid", 3000, 6, [58, 62, 60], 1.0, [(50, FSP_CA), (40, PYX), (10, OLV)], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("obsidian", "igneous", "solid", 2400, 5, [28, 24, 32], 1.2, [(75, QTZ), (15, FSP_K), (10, FSP_NA)], dict(melts_at_k=1300, boils_at_k=3000, latent_boil_k=5000)),
    ("pumice", "igneous", "solid", 650, 2, [190, 184, 170], 1.0, [(70, QTZ), (20, FSP_K), (10, FSP_NA)], dict(melts_at_k=1300, boils_at_k=3000, latent_boil_k=5000)),
    ("tuff", "igneous", "solid", 1800, 3, [176, 158, 132], 1.0, [(60, QTZ), (30, FSP_K), (10, CLAY)], dict(melts_at_k=1400, boils_at_k=3000, latent_boil_k=5000)),
    ("volcanic ash", "igneous", "powder", 900, 1, [120, 116, 112], 1.0, [(60, QTZ), (25, FSP_NA), (15, PYX)], dict(melts_at_k=1350, boils_at_k=3000, latent_boil_k=5000)),
    ("impact breccia", "igneous", "solid", 2400, 3, [124, 112, 104], 1.0, [(50, QTZ), (25, FSP_NA), (25, PYX)], dict(melts_at_k=1450, boils_at_k=3000, latent_boil_k=5000)),
    ("perovskite rock", "deep", "solid", 4100, 7, [96, 70, 64], 1.0, [(80, "MgSiO3"), (20, "FeSiO3")], dict(melts_at_k=2600, boils_at_k=3500, latent_boil_k=5000)),
    ("harzburgite", "deep", "solid", 3300, 6, [168, 170, 104], 1.0, [(85, OLV), (15, "MgSiO3")], dict(melts_at_k=1800, boils_at_k=3200, latent_boil_k=5000)),
    ("pyroxenite", "deep", "solid", 3350, 6, [52, 60, 46], 1.0, [(80, "MgSiO3"), (20, "CaMgSi2O6")], dict(melts_at_k=1650, boils_at_k=3200, latent_boil_k=5000)),
    ("plume rock", "deep", "solid", 3350, 5, [132, 84, 62], 1.1, [(60, OLV), (25, "MgSiO3"), (15, "CaAl2Si2O8")], dict(melts_at_k=1600, boils_at_k=3200, latent_boil_k=5000)),
    ("ringwoodite rock", "deep", "solid", 3650, 7, [74, 92, 168], 1.6, [(98, "Mg1.8Fe0.2SiO4"), (2, "H2O")], dict(melts_at_k=2200, boils_at_k=3400, latent_boil_k=5000)),
    ("deep pile rock", "deep", "solid", 4500, 7, [146, 48, 72], 1.4, [(100, "Mg0.6Fe0.4SiO3")], dict(melts_at_k=2700, boils_at_k=3600, latent_boil_k=5000)),
    ("eclogite", "deep", "solid", 3450, 7, [104, 84, 80], 1.0, [(55, "Mg3Al2Si3O12"), (45, "NaAlSi2O6")], dict(melts_at_k=1800, boils_at_k=3200, latent_boil_k=5000)),
    # ---------------- sedimentary ----------------
    ("sand", "sediment", "powder", 1600, 1, [214, 190, 140], 1.0, [(90, QTZ), (10, FSP_K)], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("gravel", "sediment", "powder", 1800, 2, [140, 132, 120], 1.0, [(60, QTZ), (20, FSP_K), (20, PYX)], dict(melts_at_k=1700, boils_at_k=3000, latent_boil_k=5000)),
    ("clay", "sediment", "powder", 1700, 1, [168, 120, 92], 1.0, [(80, CLAY), (20, QTZ)], dict(melts_at_k=1600, boils_at_k=3000, latent_boil_k=5000)),
    ("soil", "sediment", "powder", 1300, 1, [96, 74, 54], 1.0, [(55, QTZ), (25, CLAY), (12, "C6H10O5"), (8, HEM)], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("sandstone", "sediment", "solid", 2300, 4, [196, 150, 104], 1.0, [(85, QTZ), (10, FSP_K), (5, HEM)], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("shale", "sediment", "solid", 2500, 3, [84, 86, 92], 1.0, [(55, CLAY), (35, QTZ), (6, "C"), (4, "FeS2")], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("limestone", "sediment", "solid", 2600, 3, [200, 196, 180], 1.0, [(95, CALC), (5, QTZ)], dict(melts_at_k=1600, boils_at_k=3000, latent_boil_k=5000)),
    ("conglomerate", "sediment", "solid", 2500, 4, [150, 128, 110], 1.0, [(70, QTZ), (15, FSP_K), (15, CALC)], dict(melts_at_k=1700, boils_at_k=3000, latent_boil_k=5000)),
    ("coal", "sediment", "solid", 1350, 2, [30, 28, 26], 1.5, [(80, "C"), (10, "C6H10O5"), (6, CLAY), (4, "FeS2")], dict(melts_at_k=3800, boils_at_k=4200, latent_boil_k=5000)),
    ("rock salt", "sediment", "solid", 2170, 2, [232, 222, 220], 1.2, [(97, "NaCl"), (3, "KCl")], dict(melts_at_k=1074, boils_at_k=1738)),
    ("gypsum", "sediment", "solid", 2300, 2, [236, 232, 224], 1.0, [(100, "CaSO4(H2O)2")], dict(melts_at_k=1700, boils_at_k=3000, latent_boil_k=5000)),
    ("banded iron", "sediment", "solid", 3600, 6, [140, 60, 52], 1.8, [(45, HEM), (15, MAG), (40, QTZ)], dict(melts_at_k=1700, boils_at_k=3200, latent_boil_k=5000)),
    ("crude oil", "sediment", "liquid", 870, 0, [36, 30, 22], 2.0, [(100, "C15H32")], dict(boils_at_k=600, flow=0.35, conduct=0.1)),
    ("tar", "sediment", "liquid", 1050, 0, [20, 18, 16], 1.5, [(85, "C30H50"), (15, "S")], dict(boils_at_k=800, flow=0.05, conduct=0.1)),
    # ---------------- metamorphic ----------------
    ("marble", "metamorphic", "solid", 2700, 3, [226, 222, 214], 1.2, [(90, CALC), (10, DOLO)], dict(melts_at_k=1600, boils_at_k=3000, latent_boil_k=5000)),
    ("quartzite", "metamorphic", "solid", 2650, 7, [214, 206, 200], 1.0, [(95, QTZ), (5, MICA)], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("slate", "metamorphic", "solid", 2800, 3, [62, 66, 74], 1.0, [(60, MICA), (30, QTZ), (10, "C")], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("schist", "metamorphic", "solid", 2800, 4, [120, 118, 110], 1.0, [(50, MICA), (35, QTZ), (15, "Fe3Al2Si3O12")], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("gneiss", "metamorphic", "solid", 2750, 6, [150, 140, 136], 1.0, [(35, QTZ), (40, FSP_K), (25, MICA)], dict(melts_at_k=1550, boils_at_k=3000, latent_boil_k=5000)),
    # ---------------- ores ----------------
    ("magnetite", "ore", "solid", 5150, 6, [44, 44, 48], 1.8, [(99, MAG), (1, "V2O3")], dict(melts_at_k=1870, boils_at_k=3000, latent_boil_k=5000)),
    ("hematite", "ore", "solid", 5260, 6, [120, 40, 36], 1.8, [(100, HEM)], dict(melts_at_k=1840, boils_at_k=3000, latent_boil_k=5000)),
    ("chalcopyrite", "ore", "solid", 4200, 4, [190, 160, 60], 2.2, [(100, "CuFeS2")], dict(melts_at_k=1220, boils_at_k=2800)),
    ("native copper", "ore", "solid", 8960, 3, [184, 110, 70], 2.5, [(100, "Cu")], dict(melts_at_k=1358, boils_at_k=2835, latent_melt_k=480, latent_boil_k=12000)),
    ("galena", "ore", "solid", 7600, 3, [120, 124, 134], 2.0, [(100, "PbS")], dict(melts_at_k=1388, boils_at_k=1554)),
    ("sphalerite", "ore", "solid", 4050, 4, [116, 84, 44], 2.0, [(99.55, "ZnS"), (0.4, "CdS"), (0.03, "GaS"), (0.02, "In2S3")], dict(melts_at_k=2000, boils_at_k=2300)),
    ("cassiterite", "ore", "solid", 6950, 7, [80, 60, 50], 2.2, [(100, "SnO2")], dict(melts_at_k=1900, boils_at_k=2600)),
    ("cinnabar", "ore", "solid", 8100, 2, [196, 40, 44], 2.2, [(100, "HgS")], dict(melts_at_k=853, boils_at_k=1000)),
    ("chromite", "ore", "solid", 4700, 6, [40, 38, 40], 2.0, [(100, "FeCr2O4")], dict(melts_at_k=2400, boils_at_k=3500, latent_boil_k=5000)),
    ("ilmenite", "ore", "solid", 4700, 6, [36, 34, 38], 2.0, [(100, "FeTiO3")], dict(melts_at_k=1640, boils_at_k=3200, latent_boil_k=5000)),
    ("bauxite", "ore", "solid", 2500, 2, [176, 96, 60], 1.6, [(80, "AlO(OH)"), (15, HEM), (5, CLAY)], dict(melts_at_k=2300, boils_at_k=3200, latent_boil_k=5000)),
    ("uraninite", "ore", "solid", 8700, 5, [40, 44, 36], 3.0, [(96, "UO2"), (4, "PbO")], dict(melts_at_k=3100, boils_at_k=3800)),
    ("native silver", "ore", "solid", 10500, 3, [210, 212, 216], 3.0, [(100, "Ag")], dict(melts_at_k=1235, boils_at_k=2435, latent_melt_k=470, latent_boil_k=10000)),
    ("molybdenite", "ore", "solid", 4700, 1, [130, 134, 144], 2.2, [(99.5, "MoS2"), (0.5, "ReS2")], dict(melts_at_k=2650, boils_at_k=3500)),
    ("wolframite", "ore", "solid", 7300, 5, [52, 46, 42], 2.5, [(50, "FeWO4"), (50, "MnWO4")], dict(melts_at_k=1850, boils_at_k=3500)),
    ("stibnite", "ore", "solid", 4600, 2, [150, 154, 160], 2.0, [(100, "Sb2S3")], dict(melts_at_k=823, boils_at_k=1423)),
    ("arsenopyrite", "ore", "solid", 6100, 6, [190, 186, 176], 1.6, [(100, "FeAsS")], dict(melts_at_k=1000, boils_at_k=1500)),
    ("cobaltite", "ore", "solid", 6300, 6, [200, 190, 196], 2.5, [(100, "CoAsS")], dict(melts_at_k=1200, boils_at_k=1800)),
    ("spodumene", "ore", "solid", 3150, 7, [210, 196, 210], 2.0, [(100, "LiAlSi2O6")], dict(melts_at_k=1700, boils_at_k=3000, latent_boil_k=5000)),
    ("troilite", "ore", "solid", 4610, 4, [170, 140, 80], 1.5, [(100, "FeS")], dict(melts_at_k=1470, boils_at_k=2800)),
    ("pallasite", "ore", "solid", 4800, 6, [150, 140, 80], 2.5, [(55, "Fe9Ni"), (45, "Mg1.8Fe0.2SiO4")], dict(melts_at_k=1700, boils_at_k=3100, latent_boil_k=5000)),
    ("iron crystal", "ore", "solid", 7900, 7, [186, 180, 164], 1.5, [(100, "Fe9Ni")], dict(melts_at_k=1750, boils_at_k=3100, latent_melt_k=550, latent_boil_k=6000)),
    ("platinum-iron alloy", "ore", "solid", 8100, 5, [245, 222, 150], 4.5, [(89.06, "Fe"), (10.5, "Ni"), (0.3, "Pt"), (0.1, "Pd"), (0.02, "Ir"), (0.02, "Os")], dict(melts_at_k=2000, boils_at_k=4000, latent_melt_k=520, latent_boil_k=15000)),
    ("sulfur-rich core melt", "deep", "liquid", 6500, 0, [255, 226, 140], 1.4, [(70, "Fe"), (8, "Ni"), (14, "S"), (8, "FeO")], dict(conduct=0.5, flow=0.7)),
    ("platinum nugget", "ore", "solid", 21450, 4, [222, 222, 226], 4.0, [(82, "Pt"), (6, "Fe"), (4, "Ni"), (3, "Ir"), (2, "Pd"), (1.5, "Rh"), (1.5, "Os")], dict(melts_at_k=2041, boils_at_k=4098, latent_melt_k=520, latent_boil_k=15000)),
    # ---------------- gems and crystals ----------------
    ("quartz crystal", "gem", "solid", 2650, 7, [236, 236, 240], 2.0, [(100, QTZ)], dict(melts_at_k=1986, boils_at_k=2503)),
    ("amethyst", "gem", "solid", 2650, 7, [150, 96, 190], 3.0, [(99.9, QTZ), (0.1, HEM)], dict(melts_at_k=1986, boils_at_k=2503)),
    ("corundum", "gem", "solid", 4000, 9, [70, 90, 170], 4.0, [(99, "Al2O3"), (1, "Fe2O3")], dict(melts_at_k=2345, boils_at_k=3250)),
    ("ruby", "gem", "solid", 4000, 9, [190, 30, 60], 6.0, [(99, "Al2O3"), (1, "Cr2O3")], dict(melts_at_k=2345, boils_at_k=3250)),
    ("beryl", "gem", "solid", 2760, 8, [120, 200, 170], 4.0, [(100, "Be3Al2Si6O18")], dict(melts_at_k=1920, boils_at_k=3000, latent_boil_k=5000)),
    ("emerald", "gem", "solid", 2760, 8, [30, 170, 90], 6.0, [(99.5, "Be3Al2Si6O18"), (0.5, "Cr2O3")], dict(melts_at_k=1920, boils_at_k=3000, latent_boil_k=5000)),
    ("garnet", "gem", "solid", 3900, 7, [130, 24, 34], 2.5, [(100, "Fe3Al2Si3O12")], dict(melts_at_k=1550, boils_at_k=3000, latent_boil_k=5000)),
    ("opal", "gem", "solid", 2100, 6, [200, 210, 220], 4.0, [(92, QTZ), (8, "H2O")], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("olivine", "gem", "solid", 3300, 7, [130, 160, 60], 1.5, [(90, OLV), (10, "Fe2SiO4")], dict(melts_at_k=2100, boils_at_k=3200, latent_boil_k=5000)),
    # ---------------- carbon world ----------------
    ("graphite", "carbon", "solid", 2200, 2, [52, 54, 58], 1.4, [(100, "C")], dict(melts_at_k=3900, boils_at_k=4200)),
    ("silicon carbide", "carbon", "solid", 3210, 9, [40, 70, 72], 2.0, [(100, "SiC")], dict(melts_at_k=3000, boils_at_k=3500)),
    # ---------------- ices (with liquid partners) ----------------
    ("nitrogen ice", "ice", "solid", 1030, 1, [226, 228, 236], 1.0, [(100, "N2")], dict(melts_at_k=63, melts_to="liquid nitrogen", conduct=0.3, latent_melt_k=12)),
    ("liquid nitrogen", "ice", "liquid", 1030, 0, [200, 210, 232], 1.0, [(100, "N2")], dict(freezes_at_k=60, freezes_to="nitrogen ice", boils_at_k=77, latent_boil_k=100, conduct=0.3)),
    ("oxygen ice", "ice", "solid", 1430, 1, [190, 210, 240], 1.0, [(100, "O2")], dict(melts_at_k=54, melts_to="liquid oxygen", conduct=0.3, latent_melt_k=8)),
    ("liquid oxygen", "ice", "liquid", 1430, 0, [150, 190, 250], 1.0, [(100, "O2")], dict(freezes_at_k=52, freezes_to="oxygen ice", boils_at_k=90, latent_boil_k=110, conduct=0.3)),
    ("air snow", "ice", "powder", 1100, 1, [214, 222, 240], 1.0, [(76, "N2"), (24, "O2")], dict(melts_at_k=58, melts_to="liquid air", conduct=0.3, latent_melt_k=10)),
    ("liquid air", "ice", "liquid", 1100, 0, [180, 200, 240], 1.0, [(76, "N2"), (24, "O2")], dict(freezes_at_k=56, freezes_to="air snow", boils_at_k=80, latent_boil_k=100, conduct=0.3)),
    ("ammonia ice", "ice", "solid", 817, 1, [236, 232, 214], 1.0, [(100, "NH3")], dict(melts_at_k=195, melts_to="liquid ammonia", conduct=0.3, latent_melt_k=70)),
    ("liquid ammonia", "ice", "liquid", 817, 0, [220, 220, 196], 1.0, [(100, "NH3")], dict(freezes_at_k=192, freezes_to="ammonia ice", boils_at_k=240, latent_boil_k=300, conduct=0.3)),
    ("ethane ice", "ice", "solid", 545, 1, [222, 214, 196], 1.0, [(100, "C2H6")], dict(melts_at_k=90, melts_to="liquid ethane", conduct=0.3, latent_melt_k=40)),
    ("liquid ethane", "ice", "liquid", 545, 0, [190, 170, 130], 1.5, [(100, "C2H6")], dict(freezes_at_k=88, freezes_to="ethane ice", boils_at_k=184, latent_boil_k=200, conduct=0.3)),
    ("brine", "ice", "liquid", 1200, 0, [40, 90, 120], 1.0, [(88, "H2O"), (12, "NaCl")], dict(freezes_at_k=252, freezes_to="sea ice", boils_at_k=378, latent_boil_k=540, conduct=0.4)),
    ("sea ice", "ice", "solid", 1200, 2, [196, 222, 232], 1.0, [(88, "H2O"), (12, "NaCl")], dict(melts_at_k=255, melts_to="brine", conduct=0.3, latent_melt_k=70)),
    ("dirty ice", "ice", "solid", 1100, 2, [120, 124, 128], 1.0, [(75, "H2O"), (15, "SiO2"), (10, "C")], dict(melts_at_k=273, melts_to="muddy water", conduct=0.3, latent_melt_k=70)),
    ("muddy water", "ice", "liquid", 1100, 0, [96, 84, 66], 1.0, [(75, "H2O"), (15, "SiO2"), (10, "C")], dict(freezes_at_k=270, freezes_to="dirty ice", boils_at_k=373, latent_boil_k=500, conduct=0.4, flow=0.6)),
    ("pressure ice", "ice", "solid", 1650, 3, [150, 190, 220], 1.0, [(100, "H2O")], dict(conduct=0.3)),
    ("hot ice", "ice", "solid", 2200, 3, [200, 160, 220], 1.5, [(70, "H2O"), (20, "NH3"), (10, "CH4")], dict(conduct=0.5)),
    # ---------------- sublimating ices (solid straight to gas) ----------------
    ("dry ice", "ice", "solid", 1560, 2, [236, 240, 244], 1.0, [(100, "CO2")], dict(melts_at_k=195, melts_to="carbon dioxide", conduct=0.3, latent_melt_k=170)),
    ("carbon dioxide", "gas", "gas", 1560, 0, [210, 214, 220], 1.0, [(100, "CO2")], dict(condenses_at_k=190, condenses_to="dry ice")),
    ("sulfur dioxide frost", "ice", "solid", 1460, 1, [240, 236, 200], 1.0, [(100, "SO2")], dict(melts_at_k=198, melts_to="sulfur dioxide", conduct=0.3, latent_melt_k=120)),
    ("sulfur dioxide", "gas", "gas", 1460, 0, [230, 226, 190], 1.0, [(100, "SO2")], dict(condenses_at_k=193, condenses_to="sulfur dioxide frost")),
    ("carbon monoxide ice", "ice", "solid", 930, 1, [228, 230, 234], 1.0, [(100, "CO")], dict(melts_at_k=68, melts_to="carbon monoxide", conduct=0.3, latent_melt_k=40)),
    ("carbon monoxide", "gas", "gas", 930, 0, [214, 216, 220], 1.0, [(100, "CO")], dict(condenses_at_k=64, condenses_to="carbon monoxide ice")),
    # ---------------- comet and Titan surfaces ----------------
    ("comet dust", "surface", "powder", 600, 1, [44, 40, 38], 1.0, [(50, "Mg2SiO4"), (30, "C"), (20, "FeS")], dict(melts_at_k=1400, boils_at_k=3000, latent_boil_k=5000)),
    ("tholin", "surface", "powder", 1300, 1, [168, 96, 40], 1.5, [(100, "C6H5N")], dict(melts_at_k=600, boils_at_k=900)),
    # ---------------- gas giant and stripped core ----------------
    ("hydrogen", "gas", "gas", 70, 0, [210, 200, 220], 1.0, [(100, "H2")], {}),
    ("helium", "gas", "gas", 125, 0, [236, 226, 210], 1.0, [(100, "He")], {}),
    ("metallic hydrogen", "exotic", "liquid", 1100, 0, [200, 200, 230], 5.0, [(100, "H")], dict(conduct=0.9, flow=0.4)),
    # ---------------- crystal world ----------------
    ("resonant crystal", "exotic", "solid", 2650, 7, [170, 230, 240], 5.0, [(99, QTZ), (1, "GeO2")], dict(melts_at_k=1986, boils_at_k=2503)),
    # ---------------- volcano plumbing ----------------
    ("dolerite", "igneous", "solid", 2950, 6, [40, 60, 52], 1.0, [(50, FSP_CA), (45, PYX), (5, MAG)], dict(melts_at_k=1480, boils_at_k=3000, latent_boil_k=5000)),
    # ---------------- metal worlds ----------------
    ("kamacite", "ore", "solid", 7880, 6, [128, 130, 138], 1.4, [(94, "Fe"), (6, "Ni")], dict(melts_at_k=1770, boils_at_k=3100, latent_boil_k=6000)),
    ("taenite", "ore", "solid", 8150, 7, [206, 208, 216], 2.2, [(70, "Fe"), (30, "Ni")], dict(melts_at_k=1720, boils_at_k=3100, latent_boil_k=6000)),
    ("plessite", "ore", "solid", 7950, 6, [164, 166, 172], 1.7, [(85, "Fe"), (15, "Ni")], dict(melts_at_k=1750, boils_at_k=3100, latent_boil_k=6000)),
    ("schreibersite", "ore", "solid", 7300, 7, [188, 160, 108], 3.0, [(100, "Fe2NiP")], dict(melts_at_k=1370, boils_at_k=3000, latent_boil_k=5000)),
    ("metal grit", "surface", "powder", 5200, 2, [108, 106, 104], 1.2, [(60, "Fe9Ni"), (40, "Mg2SiO4")], dict(melts_at_k=1700, boils_at_k=3100, latent_boil_k=5000)),
    # ---------------- rocky worlds ----------------
    ("anorthosite", "igneous", "solid", 2730, 6, [206, 204, 196], 1.0, [(90, FSP_CA), (10, PYX)], dict(melts_at_k=1550, boils_at_k=3000, latent_boil_k=5000)),
    ("aquifer sandstone", "sediment", "solid", 2350, 3, [146, 116, 88], 1.1, [(72, QTZ), (8, FSP_K), (5, HEM), (15, "H2O")], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("red dust", "surface", "powder", 1500, 1, [176, 92, 58], 1.0, [(40, PYX), (25, FSP_CA), (20, HEM), (10, "MgSO4"), (5, OLV)], dict(melts_at_k=1450, boils_at_k=3000, latent_boil_k=5000)),
    # ---------------- glamour pass (interior eye candy) ----------------
    ("lherzolite", "deep", "solid", 3300, 6, [112, 138, 74], 1.0, [(60, OLV), (25, "MgSiO3"), (15, "CaMgSi2O6")], dict(melts_at_k=1700, boils_at_k=3200, latent_boil_k=5000)),
    ("dunite", "deep", "solid", 3300, 6, [156, 178, 62], 1.1, [(95, OLV), (5, "FeCr2O4")], dict(melts_at_k=1850, boils_at_k=3200, latent_boil_k=5000)),
    ("serpentinite", "deep", "solid", 2600, 4, [52, 104, 78], 1.0, [(85, "Mg3Si2O5(OH)4"), (15, MAG)], dict(melts_at_k=1500, boils_at_k=3000, latent_boil_k=5000)),
    ("majorite", "deep", "solid", 3800, 7, [156, 62, 76], 1.3, [(100, "Mg3MgSiSi3O12")], dict(melts_at_k=2300, boils_at_k=3400, latent_boil_k=5000)),
    ("ferropericlase", "deep", "solid", 4500, 6, [84, 66, 58], 1.2, [(100, "Mg0.8Fe0.2O")], dict(melts_at_k=2800, boils_at_k=3600, latent_boil_k=5000)),
    ("agate", "gem", "solid", 2600, 7, [204, 150, 120], 1.8, [(100, QTZ)], dict(melts_at_k=1986, boils_at_k=2503)),
    ("jade", "gem", "solid", 3300, 7, [72, 160, 112], 3.0, [(100, "NaAlSi2O6")], dict(melts_at_k=1300, boils_at_k=3000, latent_boil_k=5000)),
    ("lapis", "gem", "solid", 2800, 5, [38, 70, 176], 3.0, [(100, "Na3CaAl3Si3O12S")], dict(melts_at_k=1400, boils_at_k=3000, latent_boil_k=5000)),
    ("prismatic crystal", "exotic", "solid", 2700, 8, [236, 140, 236], 6.0, [(98, QTZ), (2, "GeO2")], dict(melts_at_k=2000, boils_at_k=2600)),
    ("void glass", "exotic", "solid", 2300, 6, [44, 22, 84], 4.0, [(100, QTZ)], dict(melts_at_k=1600, boils_at_k=2600)),
    # ---------------- the full table (phase 0): host minerals of the rarer elements ----------------
    # Pegmatites: the last, element-rich melt of a granite.
    ("zircon", "gem", "solid", 4650, 7, [168, 92, 58], 2.5, [(97, "ZrSiO4"), (2.5, "HfSiO4"), (0.3, "USiO4"), (0.2, "ThSiO4")], dict(melts_at_k=2820, boils_at_k=3500, latent_boil_k=5000)),
    ("thorite", "ore", "solid", 6700, 5, [206, 120, 44], 3.0, [(100, "ThSiO4")], dict(melts_at_k=2200, boils_at_k=3500, latent_boil_k=5000)),
    # Pitchblende: massive uraninite with the lead its uranium decayed into,
    # and the trace of radium that decay keeps topped up.
    ("pitchblende", "ore", "solid", 8000, 5, [30, 34, 28], 3.5, [(85, "UO2"), (15, "PbO"), (0.00003, "RaO")], dict(melts_at_k=3000, boils_at_k=3800)),
    ("columbite", "ore", "solid", 5300, 6, [42, 38, 36], 3.0, [(60, "FeNb2O6"), (25, "MnNb2O6"), (15, "FeTa2O6")], dict(melts_at_k=1800, boils_at_k=3500, latent_boil_k=5000)),
    ("tantalite", "ore", "solid", 7900, 6, [56, 42, 38], 4.0, [(70, "FeTa2O6"), (20, "MnTa2O6"), (10, "FeNb2O6")], dict(melts_at_k=1900, boils_at_k=3600, latent_boil_k=5000)),
    ("pyrochlore", "ore", "solid", 4300, 5, [138, 84, 42], 2.5, [(100, "NaCaNb2O6F")], dict(melts_at_k=1700, boils_at_k=3300, latent_boil_k=5000)),
    ("pollucite", "gem", "solid", 2900, 6, [236, 234, 228], 3.0, [(100, "Cs2Al2Si4O12(H2O)2")], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("lepidolite", "ore", "solid", 2840, 3, [186, 140, 196], 2.0, [(88, "KLi2AlSi4O10F2"), (12, "RbLi2AlSi4O10F2")], dict(melts_at_k=1450, boils_at_k=3000, latent_boil_k=5000)),
    ("thortveitite", "gem", "solid", 3500, 6, [118, 148, 124], 5.0, [(100, "Sc2Si2O7")], dict(melts_at_k=2100, boils_at_k=3300, latent_boil_k=5000)),
    ("xenotime", "ore", "solid", 4700, 4, [164, 112, 72], 3.0, [(100, "YPO4")], dict(melts_at_k=2200, boils_at_k=3300, latent_boil_k=5000)),
    # Hydrothermal veins: near, middle and far from the granite.
    ("bismuthinite", "ore", "solid", 6800, 2, [168, 170, 178], 2.2, [(100, "Bi2S3")], dict(melts_at_k=1050, boils_at_k=1700)),
    ("native bismuth", "ore", "solid", 9800, 2, [214, 186, 206], 2.5, [(100, "Bi")], dict(melts_at_k=544, boils_at_k=1837, latent_melt_k=170, latent_boil_k=6000)),
    ("clausthalite", "ore", "solid", 8100, 3, [172, 172, 184], 2.5, [(100, "PbSe")], dict(melts_at_k=1350, boils_at_k=1700)),
    ("greenockite", "ore", "solid", 4800, 3, [232, 182, 30], 2.0, [(100, "CdS")], dict(melts_at_k=2000, boils_at_k=2300)),
    ("calaverite", "ore", "solid", 9200, 3, [204, 192, 122], 6.0, [(100, "AuTe2")], dict(melts_at_k=740, boils_at_k=1300)),
    ("baryte", "ore", "solid", 4500, 3, [222, 212, 200], 1.4, [(100, "BaSO4")], dict(melts_at_k=1850, boils_at_k=3000, latent_boil_k=5000)),
    ("lorandite", "ore", "solid", 5500, 2, [156, 38, 44], 4.0, [(100, "TlAsS2")], dict(melts_at_k=570, boils_at_k=1000)),
    # Sediments and evaporites.
    ("celestine", "sediment", "solid", 3950, 3, [168, 200, 232], 1.5, [(100, "SrSO4")], dict(melts_at_k=1880, boils_at_k=3000, latent_boil_k=5000)),
    # The last of an evaporating sea: potash salt with its bromine (richer
    # than nature).
    ("bittern salt", "sediment", "solid", 1600, 2, [228, 168, 158], 1.6, [(90, "KMgCl3(H2O)6"), (10, "MgBr2")], dict(melts_at_k=540, boils_at_k=1700)),
    ("borax", "sediment", "solid", 1730, 2, [240, 238, 232], 1.6, [(100, "Na2B4O7(H2O)10")], dict(melts_at_k=1016, boils_at_k=1850)),
    ("vanadinite", "ore", "solid", 6900, 3, [196, 64, 34], 2.5, [(100, "Pb5(VO4)3Cl")], dict(melts_at_k=1300, boils_at_k=2500)),
    ("fluorite", "gem", "solid", 3180, 4, [132, 96, 196], 1.8, [(100, "CaF2")], dict(melts_at_k=1691, boils_at_k=2806)),
    ("apatite", "ore", "solid", 3190, 5, [96, 150, 140], 1.6, [(100, "Ca5(PO4)3F")], dict(melts_at_k=1900, boils_at_k=3000, latent_boil_k=5000)),
    ("rhodochrosite", "ore", "solid", 3690, 4, [214, 104, 128], 2.0, [(100, "MnCO3")], dict(melts_at_k=1200, boils_at_k=2500)),
    ("pyrolusite", "ore", "solid", 5000, 3, [40, 38, 44], 1.8, [(100, "MnO2")], dict(melts_at_k=800, boils_at_k=2500)),
    ("pentlandite", "ore", "solid", 4900, 4, [190, 170, 110], 2.5, [(100, "Fe4.5Ni4.5S8")], dict(melts_at_k=1150, boils_at_k=2500)),
    # Desert nitrate crust, with the iodate iodine is mined from.
    ("caliche", "sediment", "solid", 2000, 2, [222, 204, 172], 1.8, [(80, "NaNO3"), (12, "Ca(IO3)2"), (8, QTZ)], dict(melts_at_k=580, boils_at_k=1100)),
    # Platinum-group minerals at the base of mafic intrusions.
    ("laurite", "ore", "solid", 6400, 7, [62, 62, 72], 5.0, [(100, "RuS2")], dict(melts_at_k=2200, boils_at_k=3500, latent_boil_k=5000)),
    ("osmiridium", "ore", "solid", 21000, 7, [192, 198, 208], 8.0, [(50, "Os"), (45, "Ir"), (5, "Ru")], dict(melts_at_k=2900, boils_at_k=5000, latent_melt_k=550, latent_boil_k=15000)),
    ("braggite", "ore", "solid", 9500, 5, [182, 182, 188], 6.0, [(50, "PtS"), (35, "PdS"), (15, "NiS")], dict(melts_at_k=1400, boils_at_k=2800)),
    ("hollingworthite", "ore", "solid", 8000, 6, [202, 196, 190], 7.0, [(100, "RhAsS")], dict(melts_at_k=1300, boils_at_k=2500)),
    # Gas trapped in the rock: argon from billions of years of potassium decay,
    # with a little helium from uranium and thorium and traces of the heavier
    # noble gases; and radon seeping out of granite.
    ("radiogenic gas", "gas", "gas", 130, 0, [226, 220, 236], 3.0, [(99.585, "Ar"), (0.2, "He"), (0.2, "Ne"), (0.01, "Kr"), (0.005, "Xe")], {}),
    ("radon gas", "gas", "gas", 300, 0, [196, 232, 214], 4.0, [(70, "Ar"), (29.99999, "He"), (0.00001, "Rn")], {}),
]


def toml_value(v):
    if isinstance(v, str):
        return f'"{v}"'
    if isinstance(v, float):
        return repr(v)
    return str(v)


# Hand-written materials (data/materials.toml) that get trace elements too:
# their major-element compositions as written by hand, and their profile.
HAND = {
    "regolith": ("mafic", dict(O=0.45, Si=0.21, Fe=0.10, Mg=0.08, Al=0.07, Ca=0.06, Ti=0.03)),
    "chondrite": ("chondrite", dict(O=0.36, Fe=0.22, Si=0.18, Mg=0.14, S=0.04, Ca=0.025, Al=0.02, Ni=0.015)),
    "basalt": ("mafic", dict(O=0.44, Si=0.23, Fe=0.09, Al=0.08, Ca=0.07, Mg=0.05, Na=0.02, Ti=0.02)),
    "magma": ("mafic", dict(O=0.44, Si=0.23, Fe=0.09, Al=0.08, Ca=0.07, Mg=0.05, Na=0.02, Ti=0.02)),
    "lava-rock": ("mafic", dict(O=0.44, Si=0.23, Fe=0.09, Al=0.08, Ca=0.07, Mg=0.05, Na=0.02, Ti=0.02)),
    "peridotite": ("ultramafic", dict(O=0.44, Mg=0.25, Si=0.21, Fe=0.07, Ca=0.015, Al=0.015)),
    "kimberlite": ("ultramafic", dict(O=0.43, Mg=0.22, Si=0.18, Fe=0.07, Ca=0.05, C=0.05)),
    "iron-nickel": ("core", dict(Fe=0.905, Ni=0.09, Co=0.005)),
    "molten-iron": ("core", dict(Fe=0.85, Ni=0.08, S=0.07)),
    "cast-iron": ("core", dict(Fe=0.85, Ni=0.08, S=0.07)),
}


def patch_hand():
    import re as _re
    path = root / "data/materials.toml"
    text = path.read_text()
    for name, (prof, base) in HAND.items():
        TRACE_OF.setdefault(prof, [])
        if name not in TRACE_OF[prof]:
            TRACE_OF[prof].append(name)
        comp = fmt_comp(with_traces(name, dict(base)))
        pat = _re.compile(r'(name = "' + _re.escape(name) + r'"\n(?:(?!\[\[material\]\]).*\n)*?)composition = \{[^}]*\}')
        text, n = pat.subn(lambda m: m.group(1) + "composition = " + comp, text, count=1)
        assert n == 1, name
    path.write_text(text)


def main():
    patch_hand()
    existing = {m["name"] for m in tomllib.load(open(root / "data/materials.toml", "rb"))["material"]}
    out = ["# Generated by tools/catalogue.py: the v1 material catalogue.",
           "# Compositions are worked out from mineral formulas; edit the script, not this file.", ""]
    families = {}
    for row in CAT:
        name, fam, state, rho, hard, col, mult, parts, extra = row
        assert name not in existing, f"{name} already exists"
        families.setdefault(fam, []).append(name)
        out += ["[[material]]", f'name = "{name}"', f'state = "{state}"', f"density = {rho}", f"hardness = {hard}",
                f"colour = {col}", f"multiplier = {mult}", f"composition = {fmt_comp(with_traces(name, mix(parts)))}"]
        out += [f"{k} = {toml_value(v)}" for k, v in extra.items()]
        out.append("")
    (root / "data/materials-catalogue.toml").write_text("\n".join(out))
    print(f"{len(CAT)} materials")
    for fam, names in families.items():
        print(f"  {fam:12} {len(names):3}  {', '.join(names)}")


main()
