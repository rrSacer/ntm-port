# Energia, lote 3: motores a combustão e diesel, pistões e FT_Combustible

Conteúdo novo: **quatro máquinas** (`machine_diesel`, `machine_combustion_engine`, `machine_turbofan`, `machine_turbinegas`), **quatro conjuntos de pistões** (`piston_set__steel|dura|desh|starmetal`) e o **`ingot_starmetal`**. Total: **2.695 IDs, 422 blocos, 2.273 itens e 71 máquinas**. As máquinas seguem o barramento de máquinas (beehive/facing/honey_level 0..5, textura opaca de fallback, loot próprio, nomes en_us/pt_br do NTM). O visual-gen conserva a sobreposição visual. `machine_industrial_generator` (updateEntity vazio) e as demais fontes alternativas continuam fora.

## Pistões e o starmetal

`ItemPistons` é um item enum com um ícone e um nome por membro; cada `EnumPistonType` virou um ID com o metadata do ordinal (0 aço, 1 dura, 2 desh, 3 starmetal), textura `piston_set_<membro>.png` e empilhamento 1. A matriz de eficiência por `FuelGrade` está em `machine-recipes/combustion.json`.

O 4º pistão exige `STAR.ingot()` (`ingot_starmetal`), que não existia no catálogo. Foi importado o item original (`ItemStarmetal`) **sem receita alternativa**: a montadora usa 24 starmetal, 16 borracha, 24 placas de saturnita, 16 nióbio e 4 canos de aço rápido, exatamente como no fonte. O metadata 1–3 (astra/ursa/orion) são ícones cosméticos escondidos, sem receita nem tradução, e não foram declarados. As origens do ingot são só as do NTM, que passaram a resolver: fundição do cristal (smelting/blasting, 2 por cristal), bloco ↔ 9 lingotes, o bloco de cluster de schrabidium, anel, runa em branco, enxada e a armadura starmetal (5 itens). A receita de alto-forno `blast.starmetal` continua em skipped, pois o `ingot_meteorite` não existe.

## Construções destravadas

Todas as oito construções da montadora do fonte agora são emitidas, nenhuma em skipped: `ass.dieselgen`, `ass.combustiongen`, os quatro `ass.pistonset*`, `ass.turbofan` e `ass.gasturbine`. As variantes Ex (que usam `item_expensive`, ainda fora do catálogo) continuam omitidas. Montadora **433/198 → 441/190** (emitidas/skipped). Efeitos colaterais do `ingot_starmetal` nas outras tabelas: bigorna construção **113/144 → 114/143**, centrífuga 54 → 55, forno de arco sólido 143 → 144 e fluido 491 → 492, fundição 611 → 612 itens, triturador 624 → 625 chaves. Receitas vanilla: 10 bulk + 2 da regra de cristais (fundição e alto-forno do cristal; a cópia bulk dessas duas é descartada pelo gerador, então uma regeneração limpa reproduz o conteúdo).

## FT_Combustible e FuelGrade (`fluid_traits.json`)

`content/fluid_traits.json` ganhou `fuel_grades` (LOW, MEDIUM, HIGH, AERO, GAS; a ordem é o ordinal que as máquinas usam) e `combustible`: **43 fluidos** (HIGH 16, MEDIUM 12, GAS 7, LOW 6, AERO 2), cada um com grade, ordinal, HE por balde e linha da fonte. A tabela é separada de `flammable`: o fonte arredonda os dois valores separadamente em `registerCalculatedFuel` (`Fluids.java:935–946`) e a unidade é HE, não TU. Exemplo: o diesel tem 1.370.000 HE/balde como combustível e outro valor como inflamável. O valor final é o da última escrita do traço (construtor e depois cálculo na ordem do Java), reaproveitando o avaliador aritmético de `fuel_heat` (`pyrooven_recipes`), que agora também devolve a grade. `deferred_traits` ficou vazio. Perfil: padrões do NTM, sem sobreposições externas de JSON de traços.

## Contrato algorítmico (`machine-recipes/combustion.json`)

Schema 1, quatro `machine_config`, `recipes: []`, `skipped: []`; nenhuma duração ou potência fixa.

| Máquina | Regra preservada |
|---|---|
| Diesel | Buffer 50.000 HE, tanque 16.000 mB, queima 1 mB/tick. Eficiência MEDIUM 0,5, HIGH 0,75, AERO 0,1; LOW é excluído e GAS não tem entrada. HE/tick = trunc(floor(HE/balde ÷ 1000) × eficiência): a divisão vem **antes** da multiplicação. Só gera ligado, sem redstone, com combustível aceito. A tabela `derived_he_per_mb` traz o resultado por fluido. |
| Motor a combustão | Buffer 2.500.000 HE, tanque 24.000 mB. Combustível em décimos de mB (`fill×10+tenth`), `setting` 0..30 na GUI (o servidor aceita qualquer inteiro), `toBurn=min(fill10, setting×2)`, energia `toBurn × HE/10.000 × eficiência do pistão` em atribuição composta sobre long (truncamento). Nenhum teste de redstone. Pistão no slot 2; matriz STEEL/DURA/DESH/STARMETAL × 5 graus no JSON. |
| Turbofan | Buffer 1.000.000 HE, querosene 24.000 mB, sangue 24.000 mB. Só grade AERO: `floor(HE/balde÷1000)` por mB; queima `1+afterburner` mB/tick com `output=int32(burn×mB×(1+min(afterburner/3,4)))`; o upgrade AFTERBURN vai até 3 e o `flame_pony` força 100. Redstone em qualquer um dos quatro conectores para a geração. |
| Turbina a gás | Buffer 1.000.000 HE; tanques gás 100.000, lubrificante 16.000, água 16.000, vapor denso 160.000. Só grade GAS; consumo máximo gas 50, syngas 10, oxyhydrogen 100, reformgas 5, padrão 5 mB/tick; consumo = 5% ocioso + proporcional ao throttle, com acumulador fracionário e 1 mB de lubrificante a cada 10 ticks; energia = floor(HE/balde÷1000) por mB; temperatura de queima `floor(800 − e^(−HE/100000)×300)`; a água fervida alimenta vapor 10×. A rampa aleatória da potência instantânea não é reproduzida, está registrada como lacuna. |

**Semântica inteira do fonte:** o `output` do turbofan é um cast para int32; com `flame_pony` e combustíveis densos pode transbordar, sem correção silenciosa. Partida/parada, inércia do rotor, portas, poluição, sons, dano a entidades do turbofan e GUI dependem da lógica futura; este lote não os declara funcionais.

## Verificação

- **162 testes Python em cada seed (PYTHONHASHSEED 1 e 42)**; dez novos em `test_energy_combustion.py` (estados suportados, enum/metadata/textura/empilhamento dos pistões, starmetal só com origens do fonte, as oito construções emitidas, tabela de graus separada de flammable, diesel com divisão antes da multiplicação, matriz dos pistões, turbofan/turbina, ausência de potência/duração falsas, regeneração independente). Foram atualizadas as contagens de `test_anvil` e a lista de itens faltantes de `test_blast_furnace`. `validate-bulk.py` e `tools/recipe-gen/validate.py` passaram.
- Build/check: **6.341.578 bytes**, SHA-256 `7e4ff5a0c11d4a91ba6bda059fedd1a8f317bde79b58ad6e0db60852125ef031`.
- Exportação isolada do HEAD conjunto (`git archive`, incluindo o registro Inert `ee00986` do agente de lógica): `cargo test --workspace` verde (288 testes do ntm-core) e `cargo clippy --workspace -- -D warnings` sem warnings; a regeneração do conteúdo nesse HEAD reproduz `content/`, `hazards.json` e `furnace_recipes.json` sem diferenças.
- Prova em jogo: ver a seção abaixo.

Uma regeneração limpa a partir de um `git archive` do HEAD reproduz `content/` sem diferenças (as duas cópias bulk duplicadas do cristal, geradas na primeira passada, foram removidas por não serem idempotentes).

Reaproveitado em vez de criado: `energy_catalog`, `fluid_traits`, `fuel_heat` (estendido para devolver a grade), o molde `steam.py`/`heaters.py` para o contrato e o perfil novo `energy-combustion` do `verify-bedrock-processing.py`. Nenhum formato, gerador ou validador novo.

## Prova em jogo

A prova obrigatória usou o executável do horizon-patched, Xvfb, HORIZON_NO_AUDIO, options em zero, mute.py e perfil temporário em disco (`.agent-tmp`): **62 conferências** (repetida com o pacote final, sem as duas receitas duplicadas), /setblock dos quatro IDs com facing=north e honey_level=0 e 5, e /give de nove itens (as quatro máquinas, os quatro pistões e o ingot_starmetal) conferidos no inventário. O detector de rejeição não encontrou `content mod rejected`, `content mods not loaded`, `refused` nem `panicked`. A prova valida carregamento, IDs/estados e itens; não valida lógica funcional nem a fidelidade visual. Nenhum worldgen mudou neste lote. Evidências em `content-gen-evidence/energy-combustion/` (game.json, game-checks.json, game-client.log, machines.png, items.png); a geração e o build estão em `content-gen-evidence/energy-combustion-*.log`.

## Lógica nativa (30/10)

`crates/ntm-core/src/combustion.rs` (testes dele e `server/tests/combustion_tests.rs`), com as tabelas em `data/machines/combustion.json` (geradas por `tools/machine-gen/build.py` a partir de `fluid_traits.json` e de `machine-recipes/combustion.json`). As quatro máquinas oferecem o buffer à rede (`Role::Provider`), recebem só o seu combustível (o tipo do tanque; o identificador muda o tipo) e entregam o vapor denso (turbina) e o sangue (turbofan, que nada enche).

| Máquina | Como roda no port |
|---|---|
| Diesel | `step`: recipiente cheio do slot 0 no tanque, identificador do slot 3; ligada, sem sinal de redstone no bloco, com combustível aceito e `fill>0`, queima 1 mB e soma `trunc(floor(HE/balde ÷ 1000) × eficiência)` até 50.000. Botão 0 liga e desliga. |
| Motor a combustão | Dados 0..8 como a tela do NTM (potência, teto, tanque, teto, cor, ligado, `setting`, pistão 1..4, HE/t). Botão 0 liga; botões 100..130 põem o `setting` 0..30. Queima `min(fill10, setting×2)` décimos de mB com `power = (long)(power + toBurn × (HE/10.000) × eficiência)`; o pistão do slot 2, a matriz por grau, sem checar redstone. |
| Turbofan | Só combustível AERO, `1 + pós-combustão` mB por tick (`upgrade_afterburn_1..3` no slot 2 ou `flame_pony` = 100), `output = (int)(floor(HE/balde ÷ 1000) × mB × (1 + min(pós/3, 4)))`; sem redstone nos quatro conectores. Os efeitos no mundo (vento, fogo, gibs) e o tanque de sangue não são reproduzidos. |
| Turbina a gás | Estado 0, -1 (partida) e 1, como `TileEntityMachineTurbineGas`: partida de 580 ticks, parada em 225, o rotor e o trocador com inércia, consumo `consMax × 0,05 + consMax × aceleração` com resto fracionário, 1 mB de lubrificante a cada 10 ticks, água em vapor denso 10×. Botões 0 (partida e parada), 1 (automático) e 100..160 (o controle). |

**Redstone:** o servidor lê o sinal no bloco do diesel e nos quatro conectores do turbofan (`native_redstone`, uma chamada por dimensão), e a máquina para com qualquer um deles.

**Portas** (em `container_bridge.rs`, no referencial de quem pôs): motor a combustão e turbofan, os quatro conectores de energia e de fluido; turbina a gás, a energia em quatro células na camada de cima e os fluidos nos lados e no fim.

**Lacunas:** a poluição (a fumaça), os sons, a porta do motor, os itens carregados pelo buffer (o port não tem carga em itens), o efeito do turbofan nas entidades, o tanque de sangue, e a rampa aleatória da potência instantânea da turbina a gás, que usa o meio do passo sorteado do NTM (o resultado muda por poucos HE por tick).
