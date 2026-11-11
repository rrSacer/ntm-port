"""ModDamageSource sources the port deals, as content damage types (Onda 1, fatia 5b, decision D2).

The engine adds a content mod's `data/<ns>/damage_type` to the damage registry at world open and resolves the
damage type tags after it (`damage_source::Catalog::with_packs`). A 1.7.10 `DamageSource` maps to: `message_id` its
name (the death message `death.attack.<name>` of the Java's lang), `exhaustion` 0 when `setDamageBypassesArmor` (it
zeroes `hungerDamage`), `scaling` never (`difficultyScaled` is false unless `setDifficultyScaled`), and the
`minecraft:bypasses_armor` tag when it bypasses armor (Resistance still applies, as in the Java).
"""
# name, bypasses armor: `ModDamageSource.radiation = new DamageSource("radiation").setDamageBypassesArmor()`.
SOURCES = [
    ('radiation', True),
]


def generate(emit, languages, translations):
    for name, bypasses_armor in SOURCES:
        emit(f'data/ntm/damage_type/{name}.json', {'message_id': name, 'exhaustion': 0.0, 'scaling': 'never'})
        for loc, table in translations.items():
            languages[loc][f'death.attack.{name}'] = table[f'death.attack.{name}']
    emit('data/minecraft/tags/damage_type/bypasses_armor.json',
         {'replace': False, 'values': [f'ntm:{name}' for name, bypasses in SOURCES if bypasses]})
