#!/usr/bin/env python3
"""Writes flux_scenarios.json: the layouts the flux oracle (Java) and the Rust
tests both run. Hand-chosen to hit every branch of the stream/heat/control code."""
import json, pathlib

S = []

def col(kind, x, z, **kw):
    return {"kind": kind, "x": x, "z": z, **kw}

def rod(x, z, var="rbmk_fuel_ueu", flux=None, **kw):
    c = col("rod", x, z, rod=var, **kw)
    if flux:
        c["flux"] = flux
    return c

def scenario(name, ticks, columns, rules=None, obstacles=None, actions=None, seed=1, probes=None):
    s = {"name": name, "ticks": ticks, "seed": seed, "columns": columns}
    if rules: s["rules"] = rules
    if obstacles: s["obstacles"] = obstacles
    if actions: s["actions"] = actions
    if probes: s["probes"] = probes
    S.append(s)

# 1. rods and a moderator in a line, default dials
scenario("rod_line_moderator", 40, [rod(0, 0, flux=[100, 0.5]), col("moderator", 1, 0), rod(2, 0, flux=[50, 0.2]), rod(3, 0, "rbmk_fuel_meu", flux=[10, 1])])

# 2. a 5x5 core: fuel, moderators, a source, a control rod and blanks
grid = []
for x in range(5):
    for z in range(5):
        if (x, z) == (2, 2):
            grid.append(rod(x, z, "rbmk_fuel_po210be", moderated=True))
        elif (x + z) % 2 == 0:
            grid.append(rod(x, z, "rbmk_fuel_ueu", moderated=True, flux=[20, 0.4] if x == 0 else None) if x != 0 or z != 0 else rod(x, z, "rbmk_fuel_ueu", flux=[200, 0.4]))
        elif (x, z) == (1, 2):
            grid.append(col("control", x, z, level=0.5, target=0.5))
        elif x == 4:
            grid.append(col("blank", x, z))
        else:
            grid.append(col("moderator", x, z))
scenario("core_5x5", 60, grid, actions=[{"tick": 10, "column": [i for i, c in enumerate(grid) if c["kind"] == "control"][0], "target": 0.1}])

# 3. reflector and absorber at efficiency 1 and below 1
for eff in ("1.0", "0.5"):
    scenario("reflector_absorber_eff_" + eff.replace(".", "_"), 30,
             [col("reflector", -1, 0), rod(0, 0, flux=[300, 0.8], lid=False), col("moderator", 1, 0, ), col("absorber", 2, 0), rod(3, 0, flux=[80, 0.3]),
              col("reflector", 3, -1), col("absorber", 3, 1)],
             rules={"dialReflectorEfficiency": eff, "dialAbsorberEfficiency": eff, "dialModeratorEfficiency": eff, "dialAbsorberHeatConversion": "0.2"})

# 4. manual control rods: surge, target changes, moderated block, speed dial
c = [rod(0, 0, flux=[400, 0.5]), col("control", 1, 0, level=0.9, target=0.9, moderated=True), rod(2, 0, flux=[100, 0.1]),
     col("control", 0, 1, level=1.0, target=1.0)]
scenario("manual_control_surge", 90, c, rules={"dialControlSurgeMod": "2.0", "dialControlSpeed": "3"},
         actions=[{"tick": 3, "column": 1, "target": 0.2}, {"tick": 40, "column": 3, "target": 0.0}, {"tick": 60, "column": 1, "target": 1.0}])

# 5. automatic control rods: the three functions and the degenerate band
c = [rod(0, 0, flux=[500, 0.6], core=300, hull=300)]
for i, f in enumerate((0, 1, 2)):
    c.append(col("control_auto", i + 1, 0, level=0.5, target=0.5, level_lower=10, level_upper=90, heat_lower=20, heat_upper=60, function=f, heat=40 + 5 * i))
c.append(col("control_auto", 0, 1, level=0.3, target=0.3, level_lower=0, level_upper=100, heat_lower=20, heat_upper=20, function=0, heat=20))
c.append(col("control_auto", 0, 2, level=0.3, target=0.3, level_lower=100, level_upper=0, heat_lower=100, heat_upper=1, function=1, heat=10))
scenario("auto_control", 50, c)

# 6. powered (ReaSim) control rods
c = [rod(0, 0, flux=[300, 0.5]), col("control_reasim", 1, 0, level=0.0, target=0.0, power=12000),
     col("control_reasim_auto", 2, 0, level=0.0, target=0.0, power=30000, level_lower=0, level_upper=100, heat_lower=20, heat_upper=60, function=0, heat=50)]
scenario("powered_controls", 80, c, actions=[{"tick": 2, "column": 1, "target": 1.0}, {"tick": 50, "column": 1, "power": 20000}])

# 7. ReaSim rods: eight angled streams, bigger range
c = [col("rod_reasim", 0, 0, rod="rbmk_fuel_ueu", flux=[200, 0.5], lid=False)]
for x in range(-3, 4):
    for z in range(-3, 4):
        if (x, z) != (0, 0) and (x + z) % 3 == 0:
            c.append(col("moderator", x, z) if (x * z) % 2 else rod(x, z, "rbmk_fuel_ueu"))
scenario("reasim_rod_angles", 30, c, rules={"dialFluxRange": "7"}, seed=424242)
scenario("reasim_two_rods_reflector", 30, [col("rod_reasim", 0, 0, rod="rbmk_fuel_pu238be", flux=[50, 0.5]), col("reflector", 2, 0), col("rod_reasim", -2, -1, rod="rbmk_fuel_mox", flux=[90, 0.5], moderated=True),
                                       col("absorber", 0, 3), col("control", -3, 0, level=0.4, target=0.4)], rules={"dialFluxRange": "5"}, seed=99)

# 8. obstructions: partial and full, lidless columns, column height dial
scenario("obstacles_and_lids", 25, [rod(0, 0, flux=[300, 0.5], lid=False), col("moderator", 2, 0, lid=False), rod(4, 0, flux=[100, 0.5])],
         obstacles=[{"x": 1, "z": 0, "height": 2}, {"x": 3, "z": 0, "height": 4}, {"x": 0, "z": 1, "height": 1, "from": 2}, {"x": 0, "z": -2, "height": 4}, {"x": 0, "z": -3, "height": 3}])
scenario("tall_columns_wide_range", 20, [rod(0, 0, flux=[250, 0.5], lid=False), rod(9, 0, flux=[100, 0.5])], rules={"dialColumnHeight": "9", "dialFluxRange": "12"},
         obstacles=[{"x": 0, "z": 5, "height": 5}, {"x": 5, "z": 0, "height": 9}, {"x": -4, "z": 0, "height": 3}])

# 9. heat only: neighbours, edges, passive cooling, ReaSim boilers
c = [col("blank", 0, 0, heat=900), col("blank", 1, 0, heat=300, water=1000, steam=50), col("moderator", 0, 1, heat=100, water=33, steam=15990), col("absorber", 1, 1, heat=1200, water=16000, steam=0), col("blank", 5, 5, heat=500)]
scenario("heat_flow_default", 40, c)
scenario("heat_flow_reasim_boilers", 60, c, rules={"dialReasimBoilers": "true", "dialReasimBoilerSpeed": "0.5", "dialBoilerHeatConsumption": "0.25", "dialColumnHeatFlow": "0.7",
                                                  "dialPassiveCooling": "4", "dialPassiveCoolingInner": "0.3"})
scenario("heat_flow_zero_consumption", 10, c, rules={"dialReasimBoilers": "true", "dialBoilerHeatConsumption": "0", "dialReasimBoilerSpeed": "1"})

# 10. overheating: meltdown requested / disabled, flux dials
hot = [rod(0, 0, flux=[100, 0.5], core=3000, hull=2400, heat=1495), col("blank", 1, 0, heat=1000)]
scenario("overheat_meltdown", 30, hot)
scenario("overheat_meltdown_disabled", 30, hot, rules={"dialDisableMeltdowns": "true"})
scenario("cap_10000", 12, [rod(0, 0, "rbmk_fuel_balefire", flux=[2000, 0.5], core=20000, hull=900, heat=1400)], rules={"dialDisableMeltdowns": "true", "dialColumnHeatFlow": "0.01"})

# 11. fuel handling: insert/remove at runtime, no depletion / no xenon / reactivity mod
scenario("insert_remove_rod", 30, [col("rod", 0, 0, flux=[100, 0.5]), rod(2, 0, flux=[100, 0.5]), col("moderator", 1, 0)],
         actions=[{"tick": 5, "column": 0, "insert_rod": "rbmk_fuel_ueu"}, {"tick": 15, "column": 1, "remove_rod": True}, {"tick": 20, "column": 1, "insert_rod": "rbmk_fuel_heu235", "yield_fraction": 0.5, "xenon": 40}])
scenario("dials_no_depletion_xenon_x2", 25, [rod(0, 0, flux=[300, 0.5], xenon=30, yield_fraction=0.6), rod(1, 0, "rbmk_fuel_hep239", flux=[300, 0.5])],
         rules={"dialDisableDepletion": "true", "dialDisableXenon": "true", "dialReactivityMod": "2.0", "dialDiffusionMod": "3", "dialHeatProvision": "0.5"})
scenario("all_fuels_burn", 6, [rod(i, 0, v, flux=[150, 0.5]) for i, v in enumerate(json.load(open(pathlib.Path(__file__).parent / "fixtures/java.json"))["rods"] and [r["variable"] for r in json.load(open(pathlib.Path(__file__).parent / "fixtures/java.json"))["rods"]])] , rules={"dialDisableMeltdowns": "true"})


# 12. boilers: every steam grade, venting, the compressor, running dry
scenario("boiler_grades", 40, [col("boiler", 0, 0, heat=700, feed={"fluid": "WATER", "fill": 8000}),
                               col("boiler", 10, 0, heat=700, feed={"fluid": "WATER", "fill": 8000}, steam_tank={"fluid": "HOTSTEAM", "fill": 0}),
                               col("boiler", 20, 0, heat=900, feed={"fluid": "WATER", "fill": 8000}, steam_tank={"fluid": "SUPERHOTSTEAM", "fill": 0}),
                               col("boiler", 30, 0, heat=1400, feed={"fluid": "WATER", "fill": 800}, steam_tank={"fluid": "ULTRAHOTSTEAM", "fill": 5}),
                               col("boiler", 40, 0, heat=500, feed={"fluid": "WATER", "fill": 30}, steam_tank={"fluid": "STEAM", "fill": 0})],
         rules={"dialBoilerHeatConsumption": "0.3"})
scenario("boiler_vent_rng", 60, [col("boiler", 0, 0, heat=1000, feed={"fluid": "WATER", "fill": 10000}, steam_tank={"fluid": "STEAM", "fill": 999900}),
                                 col("boiler", 5, 5, heat=800, feed={"fluid": "WATER", "fill": 10000}, steam_tank={"fluid": "STEAM", "fill": 999990})],
         seed=2468, rules={"dialPassiveCooling": "0.0", "dialPassiveCoolingInner": "0.0"})
scenario("boiler_compressor", 12, [col("boiler", 0, 0, heat=20, steam_tank={"fluid": "STEAM", "fill": 900000}, feed={"fluid": "WATER", "fill": 0})],
         actions=[{"tick": t, "column": 0, "compress": True} for t in range(1, 8)] + [{"tick": 9, "column": 0, "set_tanks": {"feed": {"fill": 100}}}, {"tick": 10, "column": 0, "compress": True}])

# 13. heat exchangers
scenario("heater_coolant", 50, [col("heater", 0, 0, heat=700), col("heater", 8, 0, heat=700, feed={"fluid": "WATER", "fill": 16000}),
                                col("heater", 16, 0, heat=700, feed={"fluid": "OIL", "fill": 3000}), col("heater", 24, 0, heat=700, feed={"fluid": "PERFLUOROMETHYL", "fill": 9000}),
                                col("heater", 32, 0, heat=700, feed={"fluid": "AIR", "fill": 500}), col("heater", 40, 0, heat=700, feed={"fluid": "LAVA", "fill": 500})])
scenario("heater_identifier", 30, [col("heater", 0, 0, heat=400, identifier="WATER", feed={"fluid": "COOLANT", "fill": 100}), col("heater", 8, 0, heat=30, feed={"fluid": "COOLANT", "fill": 100}, identifier="OIL")],
         actions=[{"tick": 10, "column": 0, "identifier": "PERFLUOROMETHYL"}, {"tick": 20, "column": 0, "identifier": None}, {"tick": 22, "column": 1, "set_tanks": {"feed": {"fill": 0}}}])
cool = [col("blank", x, z, heat=600) for x in range(5) for z in range(5) if (x, z) != (2, 2)]
cool.append(col("cooler", 2, 2, heat=100, cold={"fill": 1000}, hot={"fill": 3500}))
scenario("cooler_window", 140, cool, actions=[{"tick": 100, "column": len(cool) - 1, "set_tanks": {"cold": {"fill": 400}, "hot": {"fill": 0}}}])

# 14. outgasser: both output kinds, gating, the progress threshold
probes = [{"ore": "ingotLithium", "item": "oracle_probe:lithium"}, {"ore": "blockCoal", "item": "oracle_probe:coal_block"}, {"ore": "ingotGold", "item": "oracle_probe:gold"}]
def outg(x, item, **kw):
    return col("outgasser", x, 0, **{"in": {"item": item, "count": 3, "meta": kw.pop("meta", 0)}, "progress": 9900, **kw})
scenario("outgasser_recipes", 40,
         [rod(0, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(1, "minecraft_1_7_10:brown_mushroom"), rod(5, 0, "rbmk_fuel_balefire", flux=[3000, 0.9]), outg(6, "oil_tar", meta=2),
          rod(10, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(11, "oracle_probe:lithium"), rod(15, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(16, "oracle_probe:coal_block"),
          rod(20, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(21, "oracle_probe:lithium", gas={"fluid": "SYNGAS", "fill": 10}),
          rod(25, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(26, "oracle_probe:gold", out={"item": "ingot_au198", "count": 64, "meta": 0}),
          rod(30, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(31, "oil_tar", meta=3), rod(35, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(36, "oracle_probe:lithium", gas={"fluid": "TRITIUM", "fill": 63900}),
          rod(40, 0, "rbmk_fuel_balefire", flux=[3000, 0.2]), outg(41, "oil_tar", meta=4, progress=0)],
         rules={"dialDisableMeltdowns": "true", "dialOutgasserSpeedMod": "1.5"}, probes=probes,
         actions=[{"tick": 15, "column": 1, "in": {"item": "minecraft_1_7_10:mushroom_stew", "count": 2, "meta": 0}}])

pathlib.Path(__file__).with_name("flux_scenarios.json").write_text(json.dumps({"scenarios": S}, indent=1) + "\n")
print(len(S), "scenarios")
