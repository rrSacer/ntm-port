#!/usr/bin/env python3
"""Executable Java oracle for the radiation sickness rolls (Onda 1, fatia 5).

`EntityEffectHandler.handleRadiationEffect(EntityLivingBase)` is cut **verbatim** from the HBM source and compiled
with minimal stubs (the entity classes it tests, `World.rand`, `HbmLivingProps`, `PotionEffect`); nothing of its logic
is translated. A player is run through it tick by tick over a sequence of doses, with `world.rand` a
`java.util.Random` of the case's seed: the effects it adds (tick, effect, ticks, amplifier) and the attacks it makes
(tick, source, amount) are what the port's `radiation::sickness_roll` and `radiation::death` must give with the same
seed (decision D1: one `java.util.Random` for the sickness).

Writes `tools/sickness-oracle/fixture.json`; `--check` compares (exit 1 when it differs). Needs `java`/`javac`.
  python3 tools/sickness-oracle/run.py --scratch DIR [--check]
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JAVA = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main/java/com/hbm'
FIXTURE = ROOT / 'tools/sickness-oracle/fixture.json'

# Doses in RAD, held for a stretch of ticks each: every band, its edges, and the two at which it kills (the clamp
# at 2500 included). One draw in 300 or 700 per tick: a few thousand ticks per band give each effect several times.
SCHEDULE = [(150, 400), (199, 400), (200, 2400), (399, 600), (400, 2400), (599, 600), (600, 2400),
            (799, 600), (800, 3000), (999, 600), (1000, 3), (2600, 2)]
SEEDS = [1, 42, 20251110, -7]


def method(text, signature):
    """The whole method starting at `signature`, braces matched."""
    start = text.index(signature)
    depth, i = 0, text.index('{', start)
    while True:
        depth += {'{': 1, '}': -1}.get(text[i], 0)
        i += 1
        if depth == 0:
            return text[start:i]


STUBS = '''import java.util.*;
class World { boolean isRemote = false; Random rand; void spawnEntityInWorld(Entity e) { } }
class Entity {
    World worldObj; boolean isDead; double posX, posY, posZ; float rotationYaw, rotationPitch;
    Entity() { } Entity(World w) { worldObj = w; }
    void setLocationAndAngles(double x, double y, double z, float yaw, float pitch) { }
    void setDead() { isDead = true; }
}
class DamageSource { final String name; DamageSource(String name) { this.name = name; } }
class PotionEffect { final int id, duration, amplifier; PotionEffect(int id, int duration, int amplifier) { this.id = id; this.duration = duration; this.amplifier = amplifier; } }
class Potion {
    final int id; final String name; Potion(int id, String name) { this.id = id; this.name = name; }
    static final Potion confusion = new Potion(9, "nausea"), moveSlowdown = new Potion(2, "slowness"),
        weakness = new Potion(18, "weakness"), poison = new Potion(19, "poison"), wither = new Potion(20, "wither");
    static String name(int id) { for (Potion p : new Potion[] {confusion, moveSlowdown, weakness, poison, wither}) if (p.id == id) return p.name; return "?" + id; }
}
class EntityLivingBase extends Entity {
    float health = 20; final List<String> log = new ArrayList<>(); int tick;
    EntityLivingBase() { } EntityLivingBase(World w) { super(w); }
    float getHealth() { return health; } void setHealth(float h) { health = h; }
    void addPotionEffect(PotionEffect e) { log.add("[" + tick + ",\\"" + Potion.name(e.id) + "\\"," + e.duration + "," + e.amplifier + "]"); }
    boolean attackEntityFrom(DamageSource s, float amount) { log.add("[" + tick + ",\\"attack:" + s.name + "\\"," + amount + "]"); return true; }
    void onDeath(DamageSource s) { }
}
class PlayerCapabilities { boolean isCreativeMode; }
class EntityPlayer extends EntityLivingBase { PlayerCapabilities capabilities = new PlayerCapabilities(); void triggerAchievement(Object a) { } }
class EntityCreeper extends EntityLivingBase { EntityCreeper(World w) { super(w); } }
class EntityCreeperNuclear extends EntityLivingBase { EntityCreeperNuclear(World w) { super(w); } }
class EntityCow extends EntityLivingBase { EntityCow(World w) { super(w); } }
class EntityMooshroom extends EntityCow { EntityMooshroom(World w) { super(w); } }
class EntityVillager extends EntityLivingBase { EntityVillager(World w) { super(w); } }
class EntityZombie extends EntityLivingBase { EntityZombie(World w) { super(w); } }
class EntityDuck extends EntityLivingBase { EntityDuck(World w) { super(w); } }
class EntityQuackos extends EntityLivingBase { EntityQuackos(World w) { super(w); } }
class ModDamageSource { static DamageSource radiation = new DamageSource("radiation"); }
class MainRegistry { static Object achRadDeath = new Object(), achRadPoison = new Object(); }
class ContaminationUtil { static boolean isRadImmune(EntityLivingBase e) { return false; } }
class HbmLivingProps { static float rad; static float getRadiation(EntityLivingBase e) { return rad; } static void setRadiation(EntityLivingBase e, float r) { rad = r; } }
'''


def java_source():
    handler = (JAVA / 'handler/EntityEffectHandler.java').read_text()
    body = method(handler, 'private static void handleRadiationEffect(EntityLivingBase entity)').replace('private static', 'static', 1)
    return STUBS + 'class EntityEffectHandler {\n' + body + '\n}\n' + '''
public class Oracle {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        World world = new World(); world.rand = new Random(seed);
        EntityPlayer player = new EntityPlayer(); player.worldObj = world;
        int tick = 0;
        for (int i = 1; i < args.length; i += 2) {
            float rad = Float.parseFloat(args[i]); int n = Integer.parseInt(args[i + 1]);
            for (int k = 0; k < n; k++, tick++) {
                player.tick = tick; HbmLivingProps.rad = rad;
                EntityEffectHandler.handleRadiationEffect(player);
            }
        }
        System.out.println("[" + String.join(",", player.log) + "]");
    }
}
'''


def run(scratch):
    if not shutil.which('javac'):
        raise SystemExit('needs a JDK')
    scratch.mkdir(parents=True, exist_ok=True)
    (scratch / 'Oracle.java').write_text(java_source())
    subprocess.run(['javac', '--release', '8', '-nowarn', '-d', str(scratch), str(scratch / 'Oracle.java')],
                   check=True, capture_output=True, text=True)
    cases = []
    for seed in SEEDS:
        args = [str(seed)] + [str(v) for rad, n in SCHEDULE for v in (rad, n)]
        out = subprocess.run(['java', '-cp', str(scratch), 'Oracle', *args], check=True, capture_output=True, text=True).stdout
        log = json.loads(out)
        cases.append({'seed': seed,
                      'effects': [e for e in log if not str(e[1]).startswith('attack:')],
                      'attacks': [e for e in log if str(e[1]).startswith('attack:')]})
    return {'source': 'EntityEffectHandler.handleRadiationEffect, verbatim', 'schedule': SCHEDULE, 'cases': cases}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    fixture = run(args.scratch)
    if args.check:
        if json.loads(FIXTURE.read_text()) != json.loads(json.dumps(fixture)):
            print('tools/sickness-oracle/fixture.json is not what the Java gives', file=sys.stderr)
            return 1
        return 0
    FIXTURE.write_text(json.dumps(fixture, separators=(',', ':')) + '\n')
    print(f"{sum(len(c['effects']) for c in fixture['cases'])} effects, {sum(len(c['attacks']) for c in fixture['cases'])} attacks")
    return 0


if __name__ == '__main__':
    sys.exit(main())
