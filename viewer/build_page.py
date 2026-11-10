#!/usr/bin/env python3
"""Build the self-contained viewer page: the template plus the compiled
generator (base64 WebAssembly), the material tables and the body specs."""
import base64, json, pathlib, sys
here = pathlib.Path(__file__).parent
data = here.parent / "data"
wasm = here / "target/wasm32-unknown-unknown/release/dft_viewer.wasm"
# Body types shown in the picker, in groups (file stem, label, group). Each
# can be drawn at any size class; tutorials, demos and test bodies are left out.
BODIES = [
    ("rubble-asteroid", "Rubble asteroid", "Small bodies"),
    ("metal-asteroid", "Metal asteroid", "Small bodies"),
    ("comet", "Comet", "Small bodies"),
    ("active-planetoid", "Volcanic planetoid", "Rocky worlds"),
    ("barren-world", "Barren world", "Rocky worlds"),
    ("dry-world", "Dry world", "Rocky worlds"),
    ("hothouse-world", "Hothouse world", "Rocky worlds"),
    ("living-world", "Living world", "Rocky worlds"),
    ("ice-moon", "Ice moon", "Icy worlds"),
    ("ocean-world", "Ocean world", "Icy worlds"),
    ("rogue-world", "Frozen rogue world", "Icy worlds"),
    ("methane-world", "Methane world", "Icy worlds"),
    ("sulfur-world", "Sulfur world", "Exotic worlds"),
    ("lava-world", "Lava world", "Exotic worlds"),
    ("iron-world", "Iron world", "Exotic worlds"),
    ("carbon-world", "Carbon world", "Exotic worlds"),
]
extra = sys.argv[1:]  # more stems, labelled by their spec name
bodies = []
for stem, label, group in BODIES + [(s, None, "More") for s in extra]:
    text = (data / "bodies" / f"{stem}.toml").read_text()
    if label is None:
        label = next(l.split("=", 1)[1].strip().strip('"') for l in text.splitlines() if l.startswith("name"))
    radius = next(float(l.split("=", 1)[1].split("#")[0]) for l in text.splitlines() if l.startswith("radius_m"))
    bodies.append({"stem": stem, "label": label, "group": group, "radius_m": radius, "text": text})
payload = {
    "wasm": base64.b64encode(wasm.read_bytes()).decode(),
    "materials": (data / "materials.toml").read_text() + "\n" + (data / "materials-catalogue.toml").read_text() + "\n" + (data / "geology.toml").read_text() + "\n" + (data / "glamour.toml").read_text(),
    "elements": (data / "elements.toml").read_text(),
    "economy": (data / "economy.toml").read_text(),
    "bodies": bodies,
}
blob = json.dumps(payload).replace("</", "<\\/")
page = (here / "page.template.html").read_text().replace("/*DATA*/", blob)
out = here / "dist" / "planet-viewer.html"
out.parent.mkdir(exist_ok=True)
out.write_text(page)
print(f"{out}: {len(page) / 1e6:.2f} MB, {len(bodies)} bodies")
# A standalone copy to open straight from disk in any browser (the published
# artifact gets its document wrapper added when it is published).
standalone = here / "dist" / "Planet Viewer.html"
standalone.write_text('<!doctype html>\n<html lang="en">\n<meta name="viewport" content="width=device-width, initial-scale=1">\n<style>html, body { margin: 0; height: 100%; }</style>\n' + page + "\n</html>\n")
print(f"{standalone}: standalone copy")
