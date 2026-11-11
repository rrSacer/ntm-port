"""The per-item status tables of docs/content-catalogo-k2.md.

Rewrites the block between the `k2-status` markers from the use tables
(`consumables.json`, `kits.json`) and the dependencies below, so the doc cannot
drift from the data: `python3 tools/content-gen/k2_status.py`.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOC = ROOT / 'docs/content-catalogo-k2.md'
DATA = ROOT / 'crates/ntm-core/data'
TABLE = json.loads((DATA / 'consumables.json').read_text())['entries']
TABLE.update(json.loads((DATA / 'kits.json').read_text())['entries'])
K2 = json.loads((ROOT / 'tools/content-gen/k2_items.json').read_text())

HIT = 'sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado'
NEEDS = {
    'ntm:coffee_radium': 'conquista `achRadium` (sistema de conquistas)',
    'ntm:chocolate_milk': 'mobs e itens soltos do raio 100 não levam dano nem empurrão (sem API de dano a mobs); `ExplosionCreator.composeEffectStandard` é um show de partículas do cliente, substituído por som e emissor vanilla; os raios usam o acaso por hash de posição do `ntm-core`, não o `world.rand`',
    'ntm:syringe_metal_stimpak': HIT, 'ntm:syringe_metal_medx': HIT, 'ntm:syringe_metal_psycho': HIT, 'ntm:syringe_metal_super': HIT,
    'ntm:syringe_taint': HIT + '; `HbmPotion.taint.performEffect` (a cada 2 ticks, com chance 1/40 por tentativa: `(amplificador + 1)` de dano `taint`, exceto em `EntityCreeperTainted` e `EntityTaintCrab`; mais a trilha de blocos taint sob `ENTITY_TAINT_TRAILS`) não tem tick de efeito de conteúdo: o efeito `ntm:taint` existe, passivo',
    'ntm:syringe_antidote': HIT, 'ntm:syringe_poison': HIT, 'ntm:syringe_awesome': HIT,
    'ntm:siox': '`HbmLivingProps.asbestos` e `blackLung` (sistema de doenças do pulmão)',
    'ntm:xanax': '`HbmLivingProps.digamma` (sistema digamma do jogador)',
    'ntm:fmn': '`HbmLivingProps.digamma` (a cegueira de 60 ticks está portada)',
    'ntm:five_htp': '`HbmLivingProps.digamma` (o efeito Stability está portado)',
    'ntm:pill_herbal': 'asbestos e black lung (a dose −100 RAD e os efeitos estão portados)',
    'ntm:pill_red': 'o `PermaSyncHandler` só desenha uma marca no cliente; o efeito `ntm:death` existe, passivo, 1 hora',
    'ntm:cigarette': 'asbestos e black lung; conquista `achNo9`',
    'ntm:crackpipe': 'black lung',
    'ntm:radaway_flush': 'o tooltip do Java diz 1000 RAD, o código soma 500 ticks (portado o código)',
    'ntm:bomb_waffle': '`ExplosionNukeSmall.explode(PARAMS_MEDIUM)`, o "muke", não tem contraparte no `ntm-core`',
    'ntm:pancake': '`ArmorFSB.hasFSBArmorIgnoreCharge` e `bj_helmet`, e `IBatteryItem` das armaduras (família de armaduras elétricas)',
    'ntm:peas': '`EntityQuackos` (entidade de mod)',
    'ntm:rag': '`onEntityItemUpdate` (rag na água vira `rag_damp`): sem gancho de tick de item solto no motor fixado',
    'ntm:iv_xp_empty': 'o XP é medido pelas barras do 26.3 (`Experience::needed`), não pelo `xpBarCap` do 1.7.10',
    'ntm:iv_xp': 'idem iv_xp_empty',
    'ntm:iv_empty': '',
    'ntm:loot_10': 'a lista `list10` do Java é vazia (nenhuma fuselagem de 10 chama `setRarity`): `rand.nextInt(0)` lança lá e aqui nada acontece',
    'ntm:loot_15': 'a lista `list15` do Java é vazia: idem loot_10',
    'ntm:apple_euphemium': '',
}
FILTER_DEP = ('instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: '
              'as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo')
INERT_GIVES = {
    'ntm:inf_water': '`FluidLoaderInfinite`', 'ntm:inf_water_mk2': '`FluidLoaderInfinite`',
    'ntm:fluid_barrel_infinite': '`FluidLoaderInfinite`', 'ntm:chlorine_pinwheel': '`FluidLoaderInfinite`',
    'ntm:gun_kit_1': 'onda de armas (`ItemRepairKit`)', 'ntm:gun_kit_2': 'onda de armas (`ItemRepairKit`)',
}
FILTERS = {'ntm:gas_mask_filter', 'ntm:gas_mask_filter_mono', 'ntm:gas_mask_filter_combo', 'ntm:gas_mask_filter_rag', 'ntm:gas_mask_filter_piss'}
DROP = '`onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; '
NOTHING = {
    'ntm:beta': DROP + 'o Java apaga o item no primeiro tick',
    'ntm:black_hole': DROP + '`EntityBlackHole`',
    'ntm:cell_anti_schrabidium': DROP + '`EntityNukeExplosionMK3.statFacFleija` e `EntityCloudFleija`',
    'ntm:cell_antimatter': DROP + '`ExplosionVNT(3).makeAmat()`',
    'ntm:pellet_antimatter': DROP + '`ExplosionVNT(20).makeAmat()`',
    'ntm:crystal_xen': DROP + '`ExplosionChaos.floater` e `move`',
    'ntm:singularity': DROP + '`EntityVortex`',
    'ntm:singularity_counter_resonant': DROP + '`EntityVortex`',
    'ntm:singularity_super_heated': DROP + '`EntityVortex`',
    'ntm:singularity_spark': DROP + '`EntityRagingVortex`',
    'ntm:particle_digamma': DROP + '`EntityQuasar`; `onUpdate` contamina com digamma (sem sistema)',
    'ntm:demon_core_open': DROP + 'troca por `demon_core_closed` e solta `screwdriver` (o item não existe)',
    'ntm:balefire_and_steel': '`ModBlocks.balefire` não existe no port',
    'ntm:plastic_bag': 'menu de inventário ligado à pilha (`InventoryPlasticBag`, 1 slot)',
    'ntm:casing_bag': 'menu de inventário ligado à pilha (`InventoryCasingBag`, 15 slots, `pushCasing`)',
    'ntm:containment_box': 'menu de inventário ligado à pilha (`InventoryLeadBox`, 20 slots)',
    'ntm:boat_rubber': '`EntityBoat` do mod (bote de borracha)',
    'ntm:key_kit': '`TileEntityLockableBase` (fechaduras e pinos)',
    'ntm:bobmazon': '`GUIScreenBobmazon` e `BobmazonOfferFactory` (loja do cliente)',
}
DONE = {
    'ntm:ams_lens': ('parcial', '`ItemLens.addInformation`', 'a barra de durabilidade (`showDurabilityBar`) não é desenhada: o gancho `native_item_bar` não está injetado; o tooltip está portado'),
    'ntm:matchstick': ('ativo', '`ItemMatch.onItemUse`', ''),
    'ntm:powder_fertilizer': ('ativo', '`ItemFertilizer.onItemUse`', 'a célula clicada cresce sempre e as outras sorteiam o seu próprio sucesso, como `fertilize(..., force)`; o motor só expõe `growCrop` (com o sorteio dentro), então a central o repete até a planta mudar (mudas, azaleias, propágulos, cogumelos e fungos), no máximo 64 vezes; o sorteio do crescimento usa o acaso do nível, não o de `world.rand`; provado em jogo: a muda clicada mudou nas 6 tentativas (avanço de estágio ou árvore) e gastou uma pilha cada, enquanto o bone meal vanilla mudou 5 de 10. Dependência nominal: API de crescimento forçado no motor (próxima frente), que tornaria exato o que hoje é a repetição de `growCrop`'),
}


def state(item):
    if item in NOTHING:
        return 'dependente', None, NOTHING[item]
    if item in DONE:
        s, origin, dep = DONE[item]
        return s, origin, dep
    if item in FILTERS:
        return 'parcial', 'ItemFilter + ArmorUtil.register', FILTER_DEP
    row = TABLE.get(item)
    if not row:
        return 'dependente', None, 'sem comportamento portado'
    deps = []
    if NEEDS.get(item):
        deps.append(NEEDS[item])
    if row.get('unported'):
        deps.append('itens Java ausentes do port: ' + ', '.join(f'`{u}`' for u in row['unported']))
    inert = sorted({g['item'] for g in row.get('gives', []) if g['item'] in INERT_GIVES})
    if inert:
        needs = sorted({INERT_GIVES[i] for i in inert})
        deps.append('entrega ' + ', '.join(f'`{i}`' for i in inert) + ' só com identidade: o uso real depende de ' + ' e '.join(needs))
    # `requires` is checked natively (`native/ntm/mixins/ntm/item_hooks.rs`,
    # `start`): the opener must be in the inventory. The opener's own mob hit
    # (`WeaponSpecial`) is not a bottle dependency, so it adds no gap here.
    status = 'parcial' if deps and item not in ('ntm:radaway_flush',) else 'ativo'
    return status, row['origin'], '; '.join(deps)


def render():
    out = ['| Item | Classe Java | Estado | Origem Java | Dependências nominais |', '|---|---|---|---|---|']
    counts = {}
    for e in K2:
        item = 'ntm:' + e['id'] if not e['id'].startswith('ntm:') else e['id']
        status, origin, dep = state(item)
        counts[status] = counts.get(status, 0) + 1
        origin = origin or f"`{e['class']}`, {e['at']}"
        out.append(f"| `{item}` | {e['class']} | {status} | {origin} | {dep or '-'} |")
    return '\n'.join(out), counts


EXTRA = [
    ('ntm:geiger_counter', 'ativo', '`ItemGeigerCounter`', 'relatório de 5 linhas e cliques a cada 5 ticks pelo `radBuf` do servidor de radiação; "radiação do chunk" é a taxa das zonas de fallout do port (`ChunkRadiationManager` não existe); texto em inglês'),
    ('ntm:dosimeter', 'ativo', '`ItemDosimeter`', 'idem, teto de 3,6 RAD/s'),
    ('ntm:digamma_diagnostic', 'parcial', '`ItemDigammaDiagnostic`', '`HbmLivingProps.digamma` não existe: imprime 0'),
    ('ntm:survey_scanner', 'parcial', '`ItemSurveyScanner`', 'marcadores de partícula do minério de bedrock, `TileEntityBedrockOre` e a viagem ao End com `entanglement_kit`'),
    ('ntm:pipette, ntm:pipette_boron, ntm:pipette_laboratory', 'parcial', '`ItemPipette`', 'capacidade por clique direito portada; encher e esvaziar exige o slot de fluido de máquinas (`IFillableItem`)'),
    ('ntm:inf_water, ntm:inf_water_mk2, ntm:fluid_barrel_infinite, ntm:chlorine_pinwheel', 'dependente', '`ItemInfiniteFluid`', '`FluidLoaderInfinite` (o slot de tanque das máquinas que os usa)'),
    ('ntm:gun_kit_1, ntm:gun_kit_2', 'dependente', '`ItemRepairKit`', 'repara armas: onda de armas (só identidade)'),
    ('ntm:ingot_meteorite, ntm:ingot_meteorite_forged, ntm:blade_meteorite, ntm:ingot_steel_dusted, ntm:ingot_chainsteel', 'parcial', '`ItemHot`', 'esfriamento de 1 por tick e o fim da tag portados; `heatUp` vem das receitas de bigorna e fundição com NBT (não portadas); no Java o `heat` é estático e vale 100 para todos'),
]

SOUND_SUBSTITUTES = {
    'random.burp': 'Comer ItemFood (burp)', 'hbm:item.syringe': 'Injeção', 'hbm:item.radaway': 'Bolsa de RadAway',
    'hbm:item.unpack': 'Abrir kit', 'hbm:player.groan': 'Fim do BDCL', 'hbm:player.cough': 'Cigarro e cachimbo',
    'hbm:entity.vomit': 'Vômito (ipecac, PTSD)', 'random.orb': 'Bolsa de XP', 'hbm:player.gulp': 'BDCL (gole nos ticks 0, 5, 10, 15, 19 e 26)',
}


def sounds():
    rs = (ROOT / 'crates/ntm-core/src/consumables.rs').read_text()
    block = rs[rs.index('pub fn sound('):rs.index('pub fn sound(') + 1600]
    rows = re.findall(r'"([\w.:]+)" => \("(minecraft:[\w.]+)", ([\d.]+), ([\d.]+), ([\d.]+)\)', block)
    events = json.loads((ROOT / 'crates/ntm-core/data/sound-events.json').read_text())['events']
    keys = set(SOUND_SUBSTITUTES) | {'hbm:item.techBoop'} | {f'hbm:item.geiger{i}' for i in range(1, 7)}
    rows += [(key, events.get(key, 'silêncio: evento ausente no original'), '1.0', '1.0', '0.0') for key in sorted(keys) if key.startswith('hbm:')]
    used = {}
    for item, row in TABLE.items():
        if row.get('sound'):
            used.setdefault(row['sound'], []).append(item)
        if row.get('special') in ('vomit',):
            used.setdefault('hbm:entity.vomit', []).append(item)
    out = ['| Item(ns) | Som Java | Evento no port | Volume / tom |', '|---|---|---|---|']
    for java, event, volume, pitch, spread in rows:
        items = used.get(java, [])
        label = ', '.join(f'`{i}`' for i in items[:6]) + (f' e mais {len(items) - 6}' if len(items) > 6 else '') or '`ntm:bdcl` (nos ticks 0, 5, 10, 15, 19 e 26 do uso: `count % 5 == 0 && count >= 10`)' if java == 'hbm:player.gulp' else ', '.join(f'`{i}`' for i in items[:6]) + (f' e mais {len(items) - 6}' if len(items) > 6 else '')
        tone = f'{volume} / {pitch}' + (f' + rand*{spread}' if float(spread) else '')
        out.append(f'| {label or "-"} | `{java}` | `{event}` | {tone} |')
    return '\n'.join(out)

POST = """### Pendências pós-Onda 0

Estas lacunas não se fecham agora porque o ponto de ligação está em arquivos da Onda 0 (`native/ntm/mixins/ntm/horizon-mixin.toml`, zona intocável). O motor já tem o que falta, sem mudança nele, no pin `f22df7b81`:

- Itens no chão (`beta`, `rag` na água, e as lacunas de `ItemDrop` que só precisam de remover ou trocar o item): `native_entity_tick_listened`, `native_entity_tick_pre` e `native_entity_tick_post` (API 146, sobre `minecraft:item`), e `native_set_item_entity` (API 42) para remover ou trocar a pilha. Ligar como injeções `HEAD` no `horizon-mixin.toml`.
- Pipetas (`ItemPipette`) e fluidos infinitos (`ItemInfiniteFluid`): encher e esvaziar precisam do slot de fluido das máquinas (`IFillableItem`, `FluidLoaderInfinite`). Ligar ao `FluidStore` e a `machine.rs` depois do merge da Onda 0.
"""


def main():
    table, counts = render()
    text = DOC.read_text()
    start, end = '<!-- k2-status:start -->', '<!-- k2-status:end -->'
    summary = ', '.join(f'{n} {k}' for k, n in sorted(counts.items()))
    extra = '\n'.join(['| Item | Estado | Origem Java | Dependências nominais |', '|---|---|---|---|'] + [f'| `{a}` | {b} | {c} | {d} |'.replace('`ntm:', '`ntm:') for a, b, c, d in EXTRA])
    block = f'{start}\n\nResumo dos {len(K2)} itens: {summary}.\n\n{table}\n\n### Itens fora dos 116, adicionados nesta onda\n\n{extra}\n\n### Sons usados e resolução dos eventos originais\n\n{sounds()}\n\n{POST}\n\n{end}'
    if start in text:
        text = re.sub(re.escape(start) + '.*?' + re.escape(end), lambda _: block, text, flags=re.S)
    else:
        text = text.rstrip('\n') + '\n\n## Estado por item (gerado)\n\n' + block + '\n'
    DOC.write_text(text)


if __name__ == '__main__':
    main()
