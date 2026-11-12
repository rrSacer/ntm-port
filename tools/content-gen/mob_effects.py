"""HbmPotion effects that the consumables test or set, as content mob effects.

The engine registers a content mod's mob effect from `data/<id>/horizon/mob_effect`
and has no tick behaviour for it, which is all these need: the Java reads them
as flags (`isPotionActive`), it does not run them. `HbmPotion.taint` also has a
`performEffect` (damage and trails); that part waits for the engine's effect tick
hooks and is listed in docs/content-catalogo-k2.md.
"""
# name, HbmPotion field, isBad, colour, icon cell in textures/gui/potions.png: 18x18 cells
# from y = 198 down, as vanilla's inventory screen reads `198 + index / 8 * 18`.
EFFECTS = [
    ('radx', 'radx', False, 0xBB4B00, (5, 0)),
    ('radaway', 'radaway', False, 0xBB4B00, (7, 0)),
    ('stability', 'stability', False, 0xD0D0D0, (2, 1)),
    ('death', 'death', False, 0x111111, (4, 1)),
    ('potion_sickness', 'potionsickness', False, 0xFF8080, (3, 1)),
    ('taint', 'taint', True, 0x800080, (0, 0)),
    # Onda 1, fatia 5c (D3): the radiation potion shows as this effect; the dose and the time left live in the
    # player's storage (ntm-core `zirnox_irradiation`), the effect is their flag.
    ('radiation', 'radiation', True, 0x84C128, (1, 0)),
]
SHEET = 'src/main/resources/assets/hbm/textures/gui/potions.png'


def generate(src, emit, emit_bytes, languages, crop_png, translations):
    sheet = (src / SHEET).read_bytes()
    for name, field, bad, color, (cx, cy) in EFFECTS:
        emit(f'data/ntm/horizon/mob_effect/{name}.json', {'category': 'harmful' if bad else 'beneficial', 'color': color})
        emit_bytes(f'assets/ntm/textures/mob_effect/{name}.png', crop_png(sheet, crop=(cx * 18, 198 + cy * 18, cx * 18 + 18, 198 + cy * 18 + 18)))
        for loc, table in translations.items():
            languages[loc][f'effect.ntm.{name}'] = table[f'potion.hbm_{field}']
