#!/usr/bin/env python3
"""The parts catalogue and unit recipes, priced by rarity.

Drones are built from named parts (Swarm Production Design: neural cores,
behaviour chips, servos...). Each part is a fixed mix of components
(wiring, electronics, frame...), and each component lists the elements it
can be made of. The rarest is the ideal and costs 1 kg per kg of
component; every step toward the common ones costs more, by how much more
common it is in Earth's crust (data/reference/crust-ppm.toml):

    x = 1 + MARKUP * log10(ppm / ppm_of_the_rarest)

so a part that ideally wants a rare element can always be made of a common
one, at a steep markup. Last of all, anything solid in the stockpile ("*")
stands in at a markup above the commonest, so no world can stall a build.

Part masses are set so a cutter is still 100 kg of parts and a hauler
2,000 kg, as before parts existed (the design's masses were first guesses).

Writes the [recipe] tables at the end of data/economy.toml.
Run: python tools/recipes.py
"""
import math, pathlib, tomllib

root = pathlib.Path(__file__).resolve().parent.parent
crust = tomllib.load(open(root / "data/reference/crust-ppm.toml", "rb"))

MARKUP = 0.5        # extra cost per tenfold more common
ANY_STEP = 2.0      # "*" (anything solid) costs this much over the commonest choice

# Component: (what it is, candidate elements). Order doesn't matter: they
# are sorted rarest first.
COMPONENTS = {
    "frame":          ("structure: light and stiff", ["Be", "Ti", "Al", "Mg", "Fe"]),
    "motors":         ("magnets and windings", ["Co", "Ni", "Fe"]),
    "wiring":         ("conductors and contacts", ["Au", "Ag", "Cu", "Al"]),
    "electronics":    ("semiconductors", ["Ga", "Ge", "In", "Si"]),
    "battery":        ("charge carriers", ["Li", "Zn", "Na", "Mg"]),
    "electrodes":     ("electrode metals", ["Co", "Ni", "Mn", "Fe"]),
    "cutting edge":   ("hard, heat-proof edge", ["W", "Ta", "Mo", "Cr", "Ti", "C", "Fe"]),
    "composites":     ("fibre and matrix", ["C", "Si", "Al"]),
    "armour":         ("tough hull metal", ["Ti", "Fe"]),
    "heat conductor": ("carries heat away", ["Ag", "Cu", "Al"]),
    # The specialists' components (sappers, heat-sink bots, tunnelers,
    # abductors, foresters).
    "fissile":        ("fissile metal for a chain reaction", ["U"]),
    "tamper":         ("dense metal that holds a blast in", ["W", "Ta", "Pb", "Fe"]),
    "initiator":      ("neutron source that starts the chain", ["Po", "Ra", "Be"]),
    "explosive":      ("shaped explosive lens", ["B", "Al", "C"]),
    "lattice":        ("multi-job semiconductor lattice", ["Ge", "In", "Ga"]),
    "optics":         ("lenses and infrared sensors", ["Te", "Se", "Ge", "In", "Ga", "Zn", "Si"]),
    "laser medium":   ("lasing crystal", ["Sc", "Y", "Ga", "Cr", "Al"]),
    "superalloy":     ("fatigue- and heat-proof alloy", ["Re", "Hf", "Nb", "Co", "Cr", "Ni"]),
    "refractory":     ("survives molten rock", ["Re", "Os", "Ir", "Hf", "Ta", "W", "Mo", "Zr"]),
    "lining":         ("sterile, inert lining", ["Pt", "Pd", "Au", "Ag", "Ti"]),
    "tendon":         ("woven tendon fibre", ["B", "C", "Si"]),
}

# Components with no stand-in: nothing else will do, and nothing solid is
# taken in their place ("*" is not added). A world without the element can
# only build them from what the fleet brought.
STRICT = {"fissile"}

# Part id: (name, what it is in the lore, [(component, kg)]). "bulk" is
# anything solid. A part's mass is the sum of its components.
PARTS = {
    "neural-core":    ("Neural core", "The drone's mind; every unit needs exactly one", [("electronics", 3), ("wiring", 1), ("composites", 1)]),
    "behaviour-chip": ("Behaviour chip", "Programs one job: cut, haul, carry, cool, bore", [("electronics", 0.6), ("wiring", 0.4)]),
    "servo":          ("Servo", "Moves a joint, leg or cutting arm", [("motors", 3), ("wiring", 1), ("frame", 1)]),
    "power-cell":     ("Power cell", "Stores beamed energy", [("battery", 6), ("electrodes", 3), ("bulk", 1)]),
    "frame-strut":    ("Frame strut", "Load-bearing skeleton", [("frame", 8)]),
    "armour-plate":   ("Armour plate", "Hull skin; heavier units carry more", [("armour", 12), ("bulk", 8)]),
    "cutting-head":   ("Cutting head", "The tooth of a cutter", [("cutting edge", 4), ("frame", 2)]),
    "hold-module":    ("Hold module", "One cargo cell of a hauler", [("frame", 120), ("composites", 50), ("bulk", 41)]),
    "heat-sink":      ("Heat sink", "Soaks and sheds heat; the heart of the heat-sink bot", [("heat conductor", 8), ("frame", 2)]),
    "lift-vane":      ("Lift vane", "Keeps flyers airborne", [("composites", 4), ("frame", 1)]),
    # The specialists' parts.
    "nuclear-charge": ("Nuclear charge", "The sapper's payload: a small fission charge; uranium, nothing else", [("fissile", 8), ("tamper", 10), ("explosive", 6), ("initiator", 0.2), ("electronics", 0.8)]),
    "detonator":      ("Detonator", "Arms the charge deep in the rock and fires it", [("electronics", 1), ("wiring", 1), ("explosive", 0.5)]),
    "blast-casing":   ("Blast casing", "Holds the charge together until it is deep enough", [("refractory", 6), ("armour", 6)]),
    "lattice-core":   ("Lattice core", "A neural core that runs several behaviour chips at once", [("lattice", 3), ("wiring", 1), ("composites", 1)]),
    "sensor-cluster": ("Sensor cluster", "A cluster of eyes: lenses and infrared sensors", [("optics", 2), ("electronics", 1), ("wiring", 0.5)]),
    "cutting-laser":  ("Cutting laser", "Cuts or burns whatever it is aimed at", [("laser medium", 3), ("optics", 2), ("heat conductor", 3), ("wiring", 1)]),
    "tripod-leg":     ("Tripod leg", "One long leg of a walker, its knee riding high", [("superalloy", 15), ("frame", 10), ("motors", 6), ("tendon", 4)]),
    "grapple-whip":   ("Grapple whip", "A barbed whip-arm that snatches live prey", [("tendon", 6), ("superalloy", 3), ("cutting edge", 1)]),
    "stasis-sac":     ("Stasis sac", "A leathery abdomen in bone hoops that keeps a catch whole", [("tendon", 20), ("lining", 6), ("composites", 10)]),
    "shear-claw":     ("Shear claw", "A saw-edged claw that cuts trees in sections", [("cutting edge", 8), ("superalloy", 8), ("motors", 4)]),
    "intake-rotor":   ("Intake rotor", "A bladed rotor that feeds cut plants in", [("cutting edge", 4), ("motors", 4), ("frame", 2)]),
    "clippings-bag":  ("Clippings bag", "A woven bag for what the claws cut", [("tendon", 15), ("composites", 10)]),
    "coolant-tentacle": ("Coolant tentacle", "Grows down through magma, freezing it to stone", [("refractory", 10), ("heat conductor", 10), ("tendon", 4)]),
    "drill-cone":     ("Drill cone", "The turning drill cone at a tunneler's head", [("cutting edge", 40), ("superalloy", 20), ("motors", 20)]),
}

# Unit: (what it is, [(part, count)]), as in the design's unit recipes.
UNITS = {
    "harvester": ("a cutter", [("neural-core", 1), ("behaviour-chip", 1), ("servo", 4), ("power-cell", 1), ("frame-strut", 2), ("armour-plate", 1), ("cutting-head", 3), ("lift-vane", 2)]),
    "hauler": ("a hauler", [("neural-core", 1), ("behaviour-chip", 1), ("servo", 6), ("power-cell", 2), ("frame-strut", 12), ("armour-plate", 6), ("hold-module", 8), ("lift-vane", 8)]),
    # Specialists, built by the Universal Drone Replicator (for testing:
    # normally they are earned through the meta tree).
    "sapper": ("a sapper: sacrificial, a small nuclear charge", [("neural-core", 1), ("behaviour-chip", 1), ("servo", 4), ("power-cell", 1), ("frame-strut", 2), ("armour-plate", 2), ("cutting-head", 2), ("lift-vane", 2), ("nuclear-charge", 1), ("detonator", 1), ("blast-casing", 1), ("sensor-cluster", 1)]),
    "tunneler": ("a tunneler: an arthropod drill", [("neural-core", 1), ("behaviour-chip", 1), ("servo", 8), ("power-cell", 3), ("frame-strut", 6), ("armour-plate", 10), ("cutting-head", 6), ("drill-cone", 1)]),
    "heatsink": ("a heat-sink bot", [("neural-core", 1), ("behaviour-chip", 1), ("servo", 2), ("power-cell", 2), ("frame-strut", 4), ("armour-plate", 4), ("heat-sink", 6), ("lift-vane", 2), ("coolant-tentacle", 6)]),
    "forester": ("a forester: a crab on three legs", [("lattice-core", 1), ("behaviour-chip", 2), ("servo", 6), ("power-cell", 3), ("frame-strut", 6), ("armour-plate", 8), ("tripod-leg", 3), ("shear-claw", 2), ("cutting-laser", 1), ("intake-rotor", 1), ("clippings-bag", 1), ("sensor-cluster", 2)]),
    "abductor": ("an abductor: a spider on three legs", [("lattice-core", 1), ("behaviour-chip", 2), ("servo", 6), ("power-cell", 3), ("frame-strut", 6), ("armour-plate", 6), ("tripod-leg", 3), ("grapple-whip", 2), ("cutting-laser", 1), ("stasis-sac", 1), ("sensor-cluster", 3)]),
}

# Parts made ahead and kept in stock: enough for this many of each unit.
AHEAD = {"harvester": 100, "hauler": 2}


def choices(comp):
    els = sorted(COMPONENTS[comp][1], key=lambda e: crust[e])
    base = crust[els[0]]
    out, last = [], 0.0
    for e in els:
        x = round(max(1.0 + MARKUP * math.log10(crust[e] / base), last + 0.1 if last else 1.0), 2)
        out.append((e, x))
        last = x
    if comp not in STRICT:
        out.append(("*", round(last + ANY_STEP, 2)))
    return out


def fmt(c):
    return ", ".join("{ el = \"%s\" }" % e if x == 1.0 else "{ el = \"%s\", x = %s }" % (e, x) for e, x in c)


def num(v):
    return str(int(v)) if float(v).is_integer() else str(v)


def part_kg(p):
    return sum(kg for _, kg in PARTS[p][2])


def main():
    lines = ["# ---- recipes: generated by tools/recipes.py (edit the script, not these) ----",
             "# Parts are made of components; each component's elements run rarest first,",
             "# x being kg of that element per kg of component, rising by %.1f for every" % MARKUP,
             "# tenfold more common it is in Earth's crust. \"*\" is anything solid in the",
             "# stockpile, most plentiful first (never for %s: nothing" % ", ".join(sorted(STRICT)),
             "# else will do). Units are built from parts.", "",
             "[recipe]",
             "ahead = { %s }" % ", ".join(f"{u} = {n}" for u, n in AHEAD.items()), ""]
    for pid, (name, lore, comps) in PARTS.items():
        lines.append("[[recipe.part]]")
        lines.append(f'id = "{pid}"')
        lines.append(f'name = "{name}"')
        lines.append(f'lore = "{lore}"')
        lines.append("make = [")
        for c, kg in comps:
            if c == "bulk":
                lines.append(f'  {{ name = "bulk", kg = {num(kg)} }},')
            else:
                lines.append(f'  {{ name = "{c}", kg = {num(kg)}, of = [{fmt(choices(c))}] }},')
        lines.append("]")
        lines.append("")
    for u, (what, parts) in UNITS.items():
        kg = sum(part_kg(p) * n for p, n in parts)
        lines.append(f"[recipe.{u}]              # {what}: {num(round(kg, 3))} kg of parts")
        lines.append("parts = [" + ", ".join(f'{{ part = "{p}", n = {n} }}' for p, n in parts) + "]")
        lines.append("")
    path = root / "data/economy.toml"
    text = path.read_text(encoding="utf-8")
    cut = text.find("# ---- recipes: generated")
    if cut < 0:
        cut = text.find("[recipe.harvester]")
    path.write_text(text[:cut].rstrip() + "\n\n" + "\n".join(lines), encoding="utf-8", newline="\n")
    for c in COMPONENTS:
        print(f"{c:15} " + "  ".join(f"{e} {x}" for e, x in choices(c)))
    for u, (_, parts) in UNITS.items():
        print(f"{u}: {sum(part_kg(p) * n for p, n in parts)} kg")


main()
