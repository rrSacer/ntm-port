# Onda 1: proteção, medicina e radiação (escopo)

Base: `main` 0e8eda47. Fontes: roadmap-100 (Onda 1, anexos A e C), `docs/content-catalogo-k2.md`,
`docs/native-item-hooks.md`, Java em `ntm-src`. Medicina, kits, RadAway/Rad-X, Geiger e dosímetro
já estão ativos na main (K2 "onda 1"); esta frente cobre o que ainda falta.

## Inventário nominal

| Fatia | IDs / sistema | Estado hoje | Fonte Java |
|---|---|---|---|
| 1. Regra de proteção | resistência de `steel_*`, `titanium_*`, `alloy_*`, `cobalt_*`, `starmetal_*` (20 peças no catálogo) e `minecraft:iron_*`/`golden_*` | 0 (tabela à mão só tem hazmat e M65) | `HazmatRegistry.initDefault` |
| 1. Revestimento | `hfr_cladding` (NBT) e mod de revestimento na armadura | ignorados | `HazmatRegistry.getCladding`, `ItemModCladding` |
| 2. Itens novos | `gas_mask`, `gas_mask_mono`, `gas_mask_olde`, `mask_rag`, `mask_piss`, `goggles`, `ashglasses`, `hazmat_paa_{helmet,plate,legs,boots}`, `liquidator_{helmet,plate,legs,boots}`, `asbestos_{helmet,plate,legs,boots}`, `jackt`, `jackt2`, `cladding_{paint,rubber,lead,desh,ghiorsium,iron,obsidian}` | fora do catálogo | `ModItemsArmor:25-32,86-89,267-269,334-336`, `ModItems:2859-2865,4266-4280`, `ArmorUtil:39-64` |
| 2. Classes de perigo | `SAND` e `LIGHT` do `HazardClass` | ausentes do enum | `ArmorRegistry`, `ArmorUtil` |
| 3. Descontaminação | `ntm:decon` | Inert (anexo A) | `TileEntityDecon` |
| 4. Radiação do mundo | campo por chunk (`ChunkRadiationHandlerSimple`, padrão), fontes no mundo, contaminação do jogador pelo chunk, linha "chunk" do Geiger/dosímetro, bloco `geiger` (fora do catálogo) com comparador e cliques | zonas de fallout aproximadas | `ChunkRadiationHandlerSimple`, `EntityEffectHandler:272`, `TileEntityGeiger`, `GeigerCounter` |
| 5. Doença e efeito | doença por sorteio (`EntityEffectHandler:219-258`), `HbmPotion.radiation` (curado por `med_bag`, `pill_iodine`, `decon`) | aproximação determinística de 5 s | `EntityEffectHandler`, `HbmPotion` |
| 6. Cinzas | `ntm:machine_ashpit` + cinzas de firebox/forno/queimador a lenha | Inert (anexo A) | `TileEntityAshpit`, `TileEntityFireboxBase`, `TileEntityMachineWoodBurner` |

## Ordem das fatias

1 → 2 → 3 → 5 → 4 → 6. Fatia 4: o coordenador escolheu (09/11) o campo por chunk fiel ao Java
(`ChunkRadiationHandlerSimple`, inclusive o bug do `clamp_float`); a troca das zonas de bomba pelos
blocos de fallout é uma sub-fatia própria, no fim, medida em cratera real antes e depois. Cada fatia entra com testes de
unidade, mutações, clippy `-D warnings` e prova em jogo.

## Fora desta onda (registrado, não esquecido)

- Armaduras com poderes (FSB: `security`, `cmb`, `schrabidium`, `euphemium`, `t51`, `ajr`, `hev`, `bj`...)
  são Onda 4. Seus valores do `HazmatRegistry` (e do `setRadResist`) e suas classes de perigo
  (`setHazardClass`) entram já na tabela gerada.
- `DamageResistanceHandler` (limiar e resistência de dano por categoria, por peça e por conjunto:
  `jackt`, aço, titânio, liga, cobalto...) é regra de combate: Onda 4, decisão do coordenador de
  10/11. Lacuna nominal: as armaduras desta onda só têm os pontos de armadura vanilla.
- Chaminés e fuligem como poluição (`SOOT`) dependem do sistema de poluição.
- `digamma_diagnostic`, asbestos e black lung dependem de `HbmLivingProps` (digamma/pulmão).
- Fontes que só existem em famílias Inert (RBMK, Watz, ICF, mísseis) emitem para o campo quando
  forem portadas.

## Defeitos conhecidos fora da frente

- `tools/content-gen/generate.py` na `main` não é idempotente: cada regeneração reescreve
  `door_metal.json`, `door_bunker.json`, `door_office.json` e `door_red.json` (apaga `door_sounds`).
  É defeito do gerador; esta frente restaura os quatro arquivos depois de cada geração e não os
  commita.
