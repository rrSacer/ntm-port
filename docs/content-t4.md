# T4 — catálogo estático restante e receitas `skipped` (parcial)

Branch `front-catalog`, a partir de `main @ 85d06d9c`.

## O que entrou

- **16 itens de combustível gasto** (`ItemDepletedFuel`) como identidades estáticas, em `tools/content-gen/catalog_t4.py` (ligado em `m3_catalog.py`): `ntm:waste_natural_uranium`, `waste_uranium`, `waste_thorium`, `waste_mox`, `waste_plutonium`, `waste_u233`, `waste_u235`, `waste_schrabidium`, `waste_zfb_mox` e as 7 `waste_plate_*`. Nome e textura vêm do Java e do `.lang`; a lógica de consumo e decaimento fica como `behavior_gap`.
- **Metadata 0 em blocos sem subtipo** (`bulk_recipes.py`, `Resolver.row`): um bloco com uma única identidade é `ItemStack(x, n, 0)` no Java. Antes, essas receitas caíam em `missing subtype`.

## Contagens (antes → depois)

| Medida | Antes | Depois |
|---|---|---|
| `ids.json` entradas | 3333 | 3349 (+16) |
| `anvil` `skipped` | 118 | 116 |
| `purex` `skipped` (fontes) | 22 | 5 (receitas 37 → 54) |
| `shredder` `skipped` | 193 | 192 (receitas 734 → 735) |
| `bulk-recipe-coverage` receitas geradas | 931 | 1115 (+184, ver nota) |
| `bulk-recipe-coverage` `gaps` | 1025 | 1024 |
| `bulk-coverage.json` `gaps` | 944 | 944 (não regenerado, ver abaixo) |

Nota: as 184 receitas novas são `data/ntm/recipe/bulk__*.json`, em `content/data/ntm/recipe/`, território da T3. Não foram copiadas para esta branch; o `generate.py` do merge as cria.

## Provas

- `tools/content-gen`: 272 testes rodaram. Os contratos de `anvil` e `purex` foram atualizados para as novas contagens. O teste `test_purex` de Naquadria deixou de injetar uma linha sintética de `waste_schrabidium`, porque o item real agora existe. Novo `test_catalog_t4.py` (3 testes) cobre os 16 itens e o lookup de metadata 0.
- Pacote: `cargo-horizon-mod build` e `check` do build compartilhado (`ntm-native-main`) geraram `ntm.hmod` (15 251 276 bytes), sem erro. Carregamento no cliente compartilhado: pacote lido, `content from 1 mod(s)`, sem `reject` nem `panic`. O processo do cliente saiu com código 1 logo após entrar no mundo; a causa não foi investigada.

## Lacunas

- **Falha conhecida fora desta frente:** `test_rbmk_oracle.test_catalog_identity_stack_and_previous_content_are_unchanged` compara `content/ids.json` byte a byte com `81a210d4`. Esta frente adiciona 16 IDs, então esse teste falha. Decisão para o coordenador: atualizar o contrato do RBMK ou congelar o `ids.json` no merge.
- **`bulk.py` não é idempotente nesta branch.** Rodá-lo reescreve `bulk.json` (−~1100 linhas), `machines_backlog.json` e `bulk-coverage.json`. Por isso não foi rodado e o `bulk-coverage.json` não reflete os 16 itens novos. Revertido.
- `sand_mix` (purex, 3 fontes): a fonte Java não declara textura, e não existe `sand_mix.png` no `ntm-src`.
- `anvil`: `siren_track:1..15`, `machine_stirling:1`, `machine_stirling_steel:1` precisam de subtipos de metadata; não portados.
- `anvil`: 18 `AnvilSmithingHotRecipe` e demais receitas com NBT por item.
- `shredder`: 147 entradas `getDustByName returns scrap` precisam do registro de pó; não portado.
- `centrifuge` (23): laços dinâmicos sobre `BedrockOreType`/`EnumBedrockOre`.
- `chemical_plant` (15): inteiros não literais (`GeneralConfig`) e fluidos condicionais.
- `stamp_book`, `BlockPipe` (variantes), `BarbedWire`, escadas, barris, `BlockLayering`, `BlockCM`: não tratados nesta rodada.
- Fechaduras (`seal_controller`, `stone_keyhole`, `stone_keyhole_meta`, `brick_forgotten_lock`, `key`, `key_red`, `key_kit`, `pin`): já estão em `ids.json`. Nada a acrescentar.

## Como reproduzir

```sh
HORIZON_RESOURCES=/home/sacer/Games/horizon/.local/horizon-client/26.3 \
  python3 tools/content-gen/generate.py --output <pasta_scratch>
cd tools/content-gen && HORIZON_RESOURCES=... python3 -m unittest test_catalog_t4 test_purex test_anvil
```
