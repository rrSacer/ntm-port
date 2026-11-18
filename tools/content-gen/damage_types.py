"""ModDamageSource sources the port deals, as content damage types (Onda 1, fatia 5b, decision D2).

The engine adds a content mod's `data/<ns>/damage_type` to the damage registry at world open and resolves the
damage type tags after it (`damage_source::Catalog::with_packs`). A 1.7.10 `DamageSource` maps to: `message_id` its
name (the death message `death.attack.<name>` of the Java's lang), `exhaustion` the constructor's 0.3, or 0 when
`setDamageBypassesArmor` (it zeroes `hungerDamage`), `scaling` never (`difficultyScaled` is false unless
`setDifficultyScaled`), and the `minecraft:bypasses_armor` tag when it bypasses armor (Resistance still applies, as in the Java).

`DamageSource.func_151519_b` asks for `death.attack.<name>.player` when the victim was last hit by a mob and falls back
to `death.attack.<name>` when that key has no translation; the Java's lang has no `.player` for these sources, so it
always shows the base text. The engine has no such fallback (an untranslated `.player` key shows raw), so the base text
is emitted under both keys.
"""
# name, bypasses armor, absolute, allowed in creative:
# `ModDamageSource.radiation = new DamageSource("radiation").setDamageBypassesArmor()`,
# `ModDamageSource.digamma = new DamageSource("digamma").setDamageIsAbsolute().setDamageBypassesArmor()
# .setDamageAllowedInCreativeMode()` (the blow of `HbmLivingProps.setDigamma` at 10).
# `isDamageAbsolute` skips `applyPotionDamageCalculations` (Resistance and the protection enchantments): the
# `minecraft:bypasses_effects` tag, which returns before both. `canHarmInCreative` is
# `minecraft:bypasses_invulnerability`.
SOURCES = [
    ('radiation', True, False, False),
    ('digamma', True, True, True),
]
TAGS = [('bypasses_armor', 1), ('bypasses_effects', 2), ('bypasses_invulnerability', 3)]


def generate(emit, languages, translations):
    for name, bypasses_armor, _, _ in SOURCES:
        emit(f'data/ntm/damage_type/{name}.json', {'message_id': name, 'exhaustion': 0.0 if bypasses_armor else 0.3,
                                                   'scaling': 'never'})
        for loc, table in translations.items():
            text = table[f'death.attack.{name}']
            languages[loc][f'death.attack.{name}'] = text
            languages[loc][f'death.attack.{name}.player'] = table.get(f'death.attack.{name}.player', text)
    for tag, flag in TAGS:
        emit(f'data/minecraft/tags/damage_type/{tag}.json',
             {'replace': False, 'values': [f'ntm:{s[0]}' for s in SOURCES if s[flag]]})
