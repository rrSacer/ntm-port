# Conteúdo da fundição

`content/machine-recipes/foundry.json` (schema 1) é o contrato combinado com o agente de máquinas. A tabela é extraída do checkout local do NTM, sem rede, por `tools/content-gen/foundry_recipes.py`, integrado ao pipeline existente do triturador e alto-forno. O gerador de máquinas converte esse arquivo para os dados do `ntm-core`.

## Materiais e itens

| Tabela | Quantidade | Contrato |
|---|---:|---|
| `materials` | 109 | IDs inteiros do NTM, nomes internos e OreDict, comportamento, cores, traits, conversões |
| Fundíveis / aditivos / outros | 78 / 4 / 27 | sem alterar as conversões dos não fundíveis |
| `items` | 609 | materiais **antes** da conversão, por item singular |
| Scraps já catalogados | 82 | `scraps__<material>`; 72 quanta por unidade |
| Regras `MatDistribution` | 44 | nenhuma chamada omitida; a chamada duplicada do minecart substitui a anterior |
| `alloys` / `disabled_alloys` | 13 / 3 | 11 ligas + hematita/malaquita; variantes GT6 desativadas |
| `molds` | 26 | 21 pequenos, 5 grandes; 452 pares molde × material com saída |
| `firebox_fuels` | 85 | 33 NTM e 52 vanilla; tempo, calor e recipiente |

Os nomes internos seguem a constante Java, sem `MAT_` e em minúsculas (`mingrade`, `dura`, `ferro`); os IDs públicos dos itens conservam as identidades já existentes (`red_copper`, `dura_steel` etc.). `colors` tem ordem **`[solidColorLight, moltenColor, solidColorDark]`**, declarada também em `color_fields`. Materiais construídos com `make` conservam as cores padrão de `NTMMaterial`, sem inventar paleta.

`quanta` contém todas as 31 formas estáticas de `MaterialShapes`, incluindo as técnicas de quantidade zero. Exemplos: quantum=1, nugget=8, billet=48, ingot=72, castplate=216, heavybarrel=432, block=648. O consumidor aplica `quantidade * convOut / convIn` com truncamento inteiro; carvão 72 vira carbono 36, e esmeralda 72 vira berílio 54.

O snapshot reutiliza `dictionary()` e `ShredderResolver`, estendendo apenas os prefixos declarados em `DictFrame` e as formas autogeradas do NTM. Não adiciona cobre vanilla moderno, deepslate ou entradas de outros mods. O JSON conserva o snapshot, suas origens e quatro colisões entre chaves.

A precedência é a de `Mats.getMaterialsFromItem`: percorre as chaves OreDict do snapshot, verifica entrada exata do minério antes de tentar prefixo **para aquela chave**, e para na primeira correspondência. Depois **soma** as entradas fixas do item e o scrap, se houver. Uma entrada fixa não substitui o resultado OreDict. `MatDistribution.registerEntry`, por outro lado, substitui uma entrada fixa anterior para o mesmo item. `item_sources`, `distribution_rules`, `ore_collisions` e `precedence` registram essas decisões; o snapshot exclui a ordem de registro de outros mods.

## Ligas e moldes

A fonte atual tem **16 declarações**, não 14: 13 ativas no perfil sem mods externos e três condicionais de GregTech 6 (`steelWrought`, `steelPig`, `steelMeteoric`). As 13 mantêm a ordem da fonte, frequência em ticks, ícone e entradas/saídas em quanta. Hematita e malaquita também são receitas selecionáveis do cadinho. As três condicionais ficam completas em `disabled_alloys`, com `requires_mod` e `enabled_by_default=false`.

Os 26 moldes têm ID estático, tamanho, custo, quantidade e saídas por material. A seleção por forma prefere o primeiro item NTM que não contenha `fragment`; depois usa o primeiro OreDict, como `ItemMold`. MoldBlock conserva os overrides de pedra e obsidiana. O molde de carimbo custa 288, enquanto a reciclagem do carimbo devolve 216; essa diferença existe na fonte e é preservada.

## Combustíveis

A firebox tem perfil próprio: aceita madeira e recipientes, ao contrário da tabela restrita do alto-forno. Coal/Coke/Lignite usam tempo ×1,25 e calor ×2; sólidos ×1,5/×3; foguete ×1,5/×5; balefire ×0,5/×15; sem categoria ×1/×1. O calor base é 100. Tempos não são cortados na capacidade térmica.

`fuel_details` registra valor base, categoria, modificadores, chaves OreDict e origem. `container` é nulo ou o item devolvido quando a pilha acaba; lava devolve `minecraft:bucket`. A tabela vanilla é um perfil explícito das identidades legadas da fornalha 1.7.10, validadas no registro 26.3; não se inferem combustíveis pelo nome moderno. Carvão vegetal fica sem categoria Coal e usa 1600 ticks/100 calor; carvão usa 2000/200. As classes vanilla ladder/button não são madeira nesse perfil.

Lacunas em `fuel_gaps`: `book_guide` e `coal_eternal` não estão no catálogo; combustíveis de outros mods/configurações, identidades exclusivamente modernas e subclasses genéricas de bloco NTM com `Material.wood` ainda não verificadas ficam de fora.

## IDs e receitas

São **34 IDs novos**: oito blocos e 26 itens. O catálogo passa a **2551 IDs**, com 351 blocos, 2200 itens e 21 máquinas.

Blocos: `ntm:machine_crucible`, `ntm:heater_firebox`, `ntm:foundry_channel`, `ntm:foundry_tank`, `ntm:foundry_outlet`, `ntm:foundry_slagtap`, `ntm:foundry_mold`, `ntm:foundry_basin`. Todos usam `minecraft:beehive`, `facing`, `honey_level=0..5`, 24 variantes, loot próprio, textura fonte `brick_fire`/`block_steel` e traduções pt_BR/en_US. Na fonte, somente cadinho, firebox e saída têm orientação; `facing` nos demais é parte do contrato uniforme do port. O fallback usa o cubo; OBJ, telas e multibloco pertencem à frente de visual.

Itens novos (prefixo `ntm:mold__`):

```text
nugget billet ingot plate wire plate_cast wire_dense blade blades stamp shell pipe
ingots plates plates_cast wires_dense block c9 c50 barrel_light barrel_heavy
receiver_light receiver_heavy mechanism stock grip
```

As seis receitas vanilla de `CraftingManager.java:922–927` passam a resolver: bacia, base de moldagem, quatro canais, tanque, saída e slagtap. A receita vanilla de `mold_base` já existia. Cadinho/firebox e gravação dos moldes usam a bigorna NTM, que ainda exige o port dessa máquina; não se criam receitas alternativas. Scraps são produzidos pela fundição; não há receita vanilla de conversão sintética. Hazards, abas e índice de receitas são regenerados.

## Limites e validação

O contrato solicitado fixa os scraps em 72: `NBT amount`/`liquid` não atravessa os slots atuais. Quantidades arbitrárias em scraps exigem suporte de estado do item; o consumidor deve registrar esse limite ao esvaziar o cadinho. O runtime de canais/moldagem pertence às etapas 2/3. Esta entrega inclui seus blocos e tabela de moldes, sem afirmar aceitação funcional dessas etapas.

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_foundry.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
/usr/local/bin/agent-run cargo test --workspace -j 3
```

Nove testes novos verificam materiais/conversões/cores, aritmética e unidades, precedência e entradas fixas, scraps, ordem/condicionais das ligas, moldes e custos, combustíveis/recipientes, grafo completo de IDs/blocos/loot e determinismo. Logs em `content-gen-evidence/foundry/`. Build/check do pacote são executados em cópia temporária sob `/tmp`, sem instalar nem executar jogo em `~/Games/horizon*`.

Validação desta entrega: nove testes da fundição, seis do alto-forno, sete do triturador e sete M3 passaram. A validação estática passou para 2551 IDs e 813 receitas vanilla registradas no pacote (inclui os outros geradores; o índice M1 conserva 235 fiéis e 10 temporárias). O pacote `dist/ntm-content-foundry.hmod` passou build/check API 0.2: 4.471.803 bytes, SHA-256 `dd06e62c9fecee0dedd413d398ec1a020736ce290e0af0fa804f8767cf92bbae`. O artefato é local/ignorado pelo git.

`cargo test --workspace -j 3` também passou: 201 testes, dois ignorados. O Rust foi testado no checkout compartilhado durante a integração do agente de máquinas; somente `hazards.json`, gerado pelos IDs novos, entra no commit de conteúdo dentro de `crates/`.

## Forno rotativo (`ntm:machine_rotary_furnace`)

Port de `TileEntityMachineRotaryFurnace` (`updateEntity`:140-227, `canProcess`:324-338, `consumeItems`:339-352) e de `RotaryFurnaceRecipes`. As 13 receitas saem de `content/machine-recipes/rotary_furnace.json` (`tools/content-gen/rotary_furnace_recipes.py`) e viram `crates/ntm-core/data/machines/rotary_furnace.json` por `tools/machine-gen/build.py`.

- 5 slots: três entradas sem ordem, o identificador de fluido (tipa o tanque de 16 000) e o combustível (`burnTime/2`, módulo de queima e calor `fuel::Module::ROTARY_FURNACE`). Vapor 12 000 e vapor gasto 120, na razão 100:1; `steamUseMult = 10·log10(velocidade) + 1`.
- O bico fica a `rot·2,875`, `y+1,25`, com `rot = dir.getRotation(DOWN)`; despeja `INGOT.q(1)` = 72 quanta por tick, `safe = true`, num raio de até 6 blocos (`molten_tick`, `foundry_server.rs`).
- **R8**: o `output` fundido não está em `held_contents`; o Java não tem `breakBlock` próprio e o perde ao quebrar. Teste `r8_breaking_rotary_furnace_loses_molten_output_and_keeps_only_slots`.
- Lacunas declaradas: fumaça e fuligem (o port não tem poluição).
