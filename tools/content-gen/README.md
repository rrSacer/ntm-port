## Triturador (2026-10-29)

O catálogo tem **2516 IDs**, incluindo `ntm:machine_shredder`; o pipeline exporta **722 receitas do triturador** com precedência fonte e contratos das três lâminas. A receita real de montagem também está disponível. A flag `--max-damage` habilita a extensão de durabilidade do fork; o padrão preserva API 0.2 base. Contratos, limites e reprodução em [docs/content-shredder.md](../../docs/content-shredder.md).

## M3: fluidos e máquinas (2026-10-29)

O catálogo tem **2515 IDs (341 blocos/2174 itens)** e **807 receitas vanilla**. O pipeline combina `m1.json`, `bulk.json`, `bombs.json` e `m3.json`; gera 610 recipientes preenchidos e exporta 499 receitas de processamento, com alternativas OreDict, labels pt_BR/en_US e lacunas por chamada fonte. Veja [docs/content-m3.md](../../docs/content-m3.md) para contratos, contagens e reprodução. `m3.json` é reconstruído da fonte automaticamente; não é necessário editar a lista à mão.

## Fechamento da expansão M2 (2026-10-29)

No fechamento do M2, o catálogo tinha **1881 IDs (334 blocos/1547 itens)** e **806 receitas vanilla**. O pipeline combinava `m1.json`, `bulk.json` e o contrato `bombs.json`; gera também o índice de máquinas de origem, chama receitas e hazards e preserva as tags exatas de radiação por taxa/bit.

O relatório atualizado, limites, backlogs, evidências e comandos de reprodução estão em [docs/content-mass.md](../../docs/content-mass.md). O pacote é `dist/ntm-content-mass.hmod`. A API 0.2 base não tem creative tabs; o pipeline agora inclui 8 tabelas e títulos para o patch creative-plus (ignoradas no motor base). Todas as execuções pesadas devem passar por `/usr/local/bin/agent-run`; para o jogo use `agent-run --user sacer` e o launcher `run-bulk-client.sh` (Xvfb :10, diretório temporário, áudio desligado).

A documentação abaixo descreve o M1 original e suas decisões, com contagens históricas.

# Gerador de conteúdo NTM → Horizon

Python 3, somente biblioteca padrão. Execute de qualquer diretório:

```sh
python3 /home/sacer/Workspace/ntm-port/tools/content-gen/generate.py
cd /home/sacer/Workspace/ntm-port/content
/home/sacer/Games/horizon/target/release/cargo-horizon-mod build
/home/sacer/Games/horizon/target/release/cargo-horizon-mod check target/horizon-mod/ntm.hmod
```

`--ids-only` publica apenas o contrato `content/ids.json`. `--source`, `--output` e `--catalog` permitem outro checkout/destino/catálogo. No destino e checkout padrão, a geração completa também chama `tools/recipe-gen/generate.py` se disponível. Os destinos alternativos recebem apenas o conteúdo desta ferramenta.

## Catálogo e identidade

`m1.json` é a lista declarativa revisável: um registro por conteúdo, com variável de origem, tipo, categoria e material. O gerador lê `ModBlocks.java`, `ModItems.java`, `Mats.java`, as texturas e os `.lang` do checkout informado. Falha se uma variável não tiver registro, se faltar textura ou se o metadata/forma material mudar.

`ids.json` tem `entries` com todos os IDs públicos, `machines` com os cinco blocos de máquina e `internal_blocks` com o token privado sem item. Cada entrada pública registra textura original, nomes, procedência dos nomes e identidade de origem. `type=block` também implica um item de bloco com o mesmo ID.

- Nomes normais conservam o caminho do registro NTM, retirando `tile.`/`item.`, e passam a `ntm:`.
- Subtipos 1.7.10 usam `<registro_original>__<material>`. Exemplo: `ntm:wire_fine__red_copper` corresponde a `hbm:item.wire_fine`, metadata `31`; `ntm:nugget_raw__titanium` corresponde a `hbm:item.nugget_raw`, metadata `2200`.
- Metadata é o ID do material em `Mats`, não um índice sequencial. O catálogo preserva `source_registry`, `source_variable`, `source_material` e `metadata` para receitas.
- As texturas são copiadas para `textures/item` e `textures/block` (singular): os diretórios plurais do NTM não entram nos atlases GUI vanilla. Fios com textura específica usam a textura original. Outros subtipos recebem o mesmo remapeamento de componentes RGB do NTM (branco/cinza 0x505050 → cores do material), gravado em PNG pelo código stdlib.
- Não se inventa minério de cobre vermelho: ele é uma liga. O lítio inclui `ore_gneiss_lithium`, o minério overworld que existe nesse checkout; não existe `ore_lithium` normal.

O conjunto atual tem 96 IDs públicos: 31 blocos e 65 itens avulsos. As 13 formas de armazenamento e os 12 minérios conservam o conjunto do original; cristais e tarugos dos materiais também entram. Os três moldes de ferro foram acrescentados por acordo com a frente de receitas; empilham até um. Sua durabilidade/consumo pertence à lógica Rust, porque a declaração de itens API 0.2 não permite declarar durabilidade.

## Barramento de máquinas

Máquinas usam `minecraft:beehive`: `facing=north|east|south|west`, `honey_level=0..5`. A fundação é dona do estado: zero desligado e 1–5 atividade/carga/progresso. O JSON de modelos troca a frente da fornalha de off para on e desenha uma barra de cinco níveis. `logic` de cada máquina em `ids.json` documenta essas propriedades. A API não permite inventar nomes de propriedades; `honey_level` é o nome técnico do barramento.

IDs: `ntm:machine_wood_burner`, `ntm:machine_battery`, `ntm:machine_electric_furnace_off`, `ntm:machine_press`, `ntm:machine_epress`. O sufixo `_off` continua no ID da fornalha por preservar a identidade original; a versão acesa é um estado do mesmo bloco. `machine_press` é a Burner Press original, alimentada por combustível. `red_cable` é cubo cheio sem propriedades. `logic_token` usa composter/level 0–8, sem item, loot, receita ou tradução.

As máquinas são cubos JSON. As prensas recebem seus ícones originais compostos sobre uma carcaça opaca de aço; o gerador usa um recorte da textura de seu modelo original. Não há conversão de OBJ/TESR. Colisão é um cubo inteiro, inclusive nas aproximações de máquinas que originalmente tinham formas diferentes.

A segurança do template é empírica nesta versão do Horizon, não uma promessa para versões futuras. Os scripts verificam clique vazio, tesoura, garrafa, honeycomb, bone meal, sementes, glowstone e isqueiro; vizinhos/redstone; abelha próxima e 1000 ticks; colisão lateral e superior; orientação na colocação; quebra com ferramenta e drop próprio. Interação genérica do item continua possível (por exemplo, glowstone pode colocar um bloco vizinho). O estado da máquina fica intacto e nenhum menu/colheita de mel é herdado.

## Loot e idiomas

Cada bloco público tem loot próprio. Minérios de fluorita, salitre e enxofre dropam 2–4 itens, usando o formato 26.3 (`modifier`/`type`); amianto dropa sua fibra (`ingot_asbestos`). Os outros minérios dropam o minério, como o NTM base. Não há tratamento especial de Fortune/Silk Touch. Máquinas e armazenamento dropam a si mesmos.

`en_us` e `pt_br` vêm dos `.lang` do NTM; traduções ausentes/iguais ao inglês são complementadas em português. As tabelas `name_sources` dizem quais foram traduzidas no port.

A ferramenta registra arquivos próprios em `.content-gen-files.json` e só limpa arquivos anteriormente gerados por ela. Receitas, tags e worldgen de outras frentes são preservados. API 0.2 neste checkout aceita `properties.sounds` com **ID de bloco** (`minecraft:iron_block`), apesar de exemplos antigos da documentação usarem `sound` ou nomes de conjuntos de som.

## Verificação em cliente mudo

`run-client.sh` cria novos diretórios em `/tmp`, usa options master/music=0, define `HORIZON_NO_AUDIO=1` e inicia uma tentativa adicional de mute PipeWire restrita ao diretório desse cliente. O áudio não chega a abrir um stream quando `HORIZON_NO_AUDIO` funciona; isso fica registrado em `content-gen-evidence/audio-muted.log`. O cliente é offscreen, não toca instalações do jogador e roda sob `sacer`.

```sh
bash tools/content-gen/run-client.sh > content-gen-evidence/client.log 2>&1
# Em outro terminal:
python3 tools/content-gen/verify-game.py
python3 tools/content-gen/capture-gallery.py
python3 tools/content-gen/ctl.py quit
```

`verify-game.py` grava `verification.json`; `capture-gallery.py` grava quatro PNGs e `gallery.json`. `control.jsonl` preserva comandos/respostas completos. As imagens devem ser avaliadas junto dos dados; o teste de loot usa quebra real em survival e coleta de itens. Não modifica o código do Horizon e não faz commits.

## Geração de minérios integrada

`m1.json.worldgen` declara 11 cadeias, uma por minério normal do Overworld. O módulo `worldgen.py` gera `structure_set → jigsaw → template_pool(feature_pool_element) → placed_feature → configured_feature(ore)`. São 55 arquivos próprios; a regeneração os rastreia junto dos assets.

Cada estrutura tem `size:1`, filtro `#minecraft:is_overworld`, etapa `underground_ores` e terreno sem adaptação. O set usa `random_spread`, spacing 1, separation 0 e salt distinto e estável. A feature usa `count` por chunk do NTM, `in_square`, offset 8/0/8 e altura uniforme inclusiva. Não há `minecraft:biome` no placement. Os campos de minério ficam na raiz, e o estado usa `{"id":"ntm:ore_*"}`.

Tentativas/tamanho/faixas vêm dos defaults efetivos documentados em `docs/ore-distribution.md`, incluindo urânio=7 e amianto=2. Alvos são matches exatos de stone e deepslate. As alturas positivas legadas são mantidas; não se acrescenta faixa negativa. Na pequena transição de deepslate usa-se o mesmo ID/arte legado. Criar uma família de arte deepslate e reequilibrar a profundidade negativa ficam para outro marco.

Não se gera cobre vermelho (é liga), nem lítio sem o substrato `stone_gneiss`; `ore_gneiss_lithium` continua registrado e disponível por comando/receita apropriada. Nether, gnaisse, clusters e cavidades especiais não integram esta tradução. `ids.json.worldgen_gaps` expõe essas escolhas aos outros agentes.

Verificação reprodutível, sempre sequencial por usar a porta dedicada 25731:

```sh
python3 tools/content-gen/verify-worldgen.py
python3 tools/content-gen/verify-worldgen.py --diagnostic
```

O primeiro comando cria dois mundos dedicados com seed 1768123, pregera a mesma região e compara todos os IDs/coordenadas dos minérios nos 81 chunks centrais, com halo de quatro chunks. O teste estrito registra e retorna falha se houver diferenças; a limitação M1 de duas sobreposições foi aceita e está documentada em `docs/ore-distribution.md`. O segundo constrói quatro pacotes diagnósticos separados com cubos de seis substratos conhecidos e aplica os mesmos onze configs de minério, isolando os controles positivos em lotes para evitar competição entre materiais e alturas acima do heightmap de terreno. Isso evita interpretar uma comparação dos estados finais com vanilla como prova de substituição: estruturas e decoração vanilla podem ter ordens diferentes. As contagens e saves ficam em `content-gen-evidence/worldgen/`, com caminhos para homes temporários preservados.

## Alto-forno

O pipeline também exporta `content/machine-recipes/blast_furnace.json`: receitas em ordem, alternativas OreDict e a tabela restrita de combustível. Veja [contrato, lacunas e reprodução](../../docs/content-blast-furnace.md). O bloco cúbico usa `brick_fire`; modelo/tela/multibloco pertencem ao visual-gen.
