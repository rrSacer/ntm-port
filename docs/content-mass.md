# Importação em massa de conteúdo NTM (2026-10-29)

Pacote: `dist/ntm-content-mass.hmod`, content API 0.2, versão 0.2.0.
SHA-256: `3ddb7a5aeb41f62904d9114f236202b8ebc1b399fd811a2867031be215093111`. O intermediário de materiais permanece em `dist/ntm-materials-intermediate.hmod` (706 IDs).

O catálogo foi ampliado de 96 para **1881 IDs públicos**: **334 blocos com item e 1547 itens avulsos**. Os 96 IDs M1 foram preservados. Isso é uma entrega de conteúdo e **não equivale a um port funcional 100% completo**.

| Grupo | IDs |
| --- | ---: |
| Materiais, formas, minérios e blocos de recursos | 1048 |
| Componentes/peças | 575 |
| Decorativos/estruturais | 180 |
| Ferramentas comuns / armaduras comuns | 13 / 20 |
| Bombas / componentes de bomba / superfícies fallout | 11 / 19 / 6 |
| Máquinas M1 / cabo / moldes M1 | 5 / 1 / 3 |

São 60 IDs de minério em bloco, 320 minérios processados em item, 93 lingotes, 61 pepitas, 104 pós, 62 placas, 24 fios, 54 tarugos, 30 cristais, 54 fragmentos e 85 sucatas. Há 814 identidades de metadata separadas. Todos os IDs têm chave en_us/pt_br e textura de origem; nomes de variantes são derivados dos lang/enum/Mats, com fallbacks identificados no catálogo. Subtipos usam `__material` ou `__enum`; minério moderno de bedrock conserva `grade << 4 | type` em `metadata`.

## Contratos e decisões

- `tools/content-gen/m1.json` continua como contrato do M1. `bulk.json` contém a expansão declarativa; `bombs.json` sobrepõe os campos combinados com a frente de bombas. `generate.py` une os catálogos e preserva saídas de outros geradores.
- As texturas entram nos atlas `item/`/`block/` singulares. Mats aplica o remapeamento RGB original, materiais de cor única recebem multiplicação, e minérios processados conservam overlays por grau/tipo. Não houve conversão de OBJ/TESR. Bombas nucleares são ícones compostos sobre cubos de aço.
- `item.fallout` e `tile.fallout` colidem ao retirar o prefixo legado. O bloco fica `ntm:fallout` (snow/layers); o item é **`ntm:fallout__item`**, mantendo `source_registry`. Balefire conserva o nome real **`ntm:nuke_fstbmb`**.
- Os 11 blocos `ids.json.bombs` usam `minecraft:redstone_lamp`, `light:0`, e ambos os estados `lit` usam o mesmo modelo. `logic.kind=bomb`, `powered=lit`. A explosão/montagem é responsabilidade da frente de lógica.
- 33 equipamentos comuns usam templates vanilla: aço/titânio/cobalto → ferro; outras famílias → diamante ou netherite conforme o catálogo. Durabilidade, dano/velocidade, proteção, encantabilidade, reparo e aparência da armadura equipada seguem o template. **Não são estatísticas NTM equivalentes**. `equipment_gap` preserva a declaração/definição de material da fonte e lista essas diferenças. Habilidades especiais e hazmat permanecem no backlog.
- Vidros CT/decorativos CT usam a face isolada do original; não há stitching por vizinhança. Alexandrita usa o primeiro frame, sem cor sensível à luz. Algumas superfícies de minérios/armazenamento conservam identidade e arte, com gases/calor/propagação e efeitos especiais explicitamente ausentes. Propriedades físicas gerais dos blocos continuam a aproximação do pipeline M1.
- Loot foi traduzido para recursos comuns: enxofre/salitre/fluorita 2–4, cobalto 4–9 e cobalto Nether 5–12, fragmentos/cristais e metadados aplicáveis. Fortune/Silk Touch, drops especiais complexos, XP e regras de quebra por ferramentas NTM não têm paridade completa. Vidros que não dropam na fonte recebem loot vazio; isso não implementa Silk Touch.
- Minérios Nether e End são conteúdo registrado. `ore_tikite` tem origem End (`HbmWorldGen.java:551`). **Nenhuma worldgen adicional foi inventada:** permanecem as 11 cadeias overworld M1, com a limitação de sobreposição já documentada em `ore-distribution.md`. Gnaisse/depth/meteoritos/Sellafield/geração especial e distribuição de Nether/End continuam lacunas.

## Receitas e radiação

Há **806 arquivos de receita vanilla**: 235 do pipeline anterior e **571 importados** neste lote (370 shaped, 157 shapeless, 22 smelting, 22 blasting adicionais). Blasting é a extensão 26.3 já adotada no port. Não foram criadas novas pontes temporárias; as **10 pontes M1 preexistentes** continuam identificadas por `m1_temp__`.

`bulk_recipes.py` resolve o subconjunto literal de `addRecipeAuto`, `addShapelessAuto`, `GameRegistry.addRecipe/addShapelessRecipe/addSmelting`, Shaped/ShapelessOreRecipe e pares minerais. Ingredientes OreDictionary tornam-se tags locais de membros realmente catalogados. Condicionais, loops e expressões não resolvidas não viram receitas inventadas: **1319 chamadas** estão em `content/bulk-recipe-coverage.json` com origem/motivo (incluem chamadas a helpers e saídas fora do escopo). Isso não é promessa de cobertura total das receitas Java.

**681 IDs** têm `source_recipe_machine` como saída exata indexada ou forma autogerada. Quando a sintaxe não permite identificar a saída, `source_recipe_references` guarda referências claramente marcadas como possíveis entradas/saídas. O catálogo não afirma que mencionar um item em uma receita o torna saída.

`hazard-gen/extract.py` agora extrai constantes aritméticas e registros explícitos de HazardRegistry/OreDictManager: todos os 1881 IDs estão em `hazards.json`, 228 com radiação positiva. Subtipos sem registro efetivo resolvido conservam zero com lacuna explícita; isso inclui sucatas/formas que exigem NBT ou tratamento adicional. Não se inventaram taxas por semelhança de nomes.

O extrator emite **97 tags por taxa exata e 27 tags por bit** em `data/ntm/tags/item/rad/`, com IDs diretos, ordenados, `replace:false`. `crates/ntm-core/data/radiation_buckets.json` publica taxas e bits. Esse contrato foi combinado com a fundação para substituir a amostragem linear por item, preservando a soma exata. A lógica Rust e sua aceitação são responsabilidade daquela frente.

## Backlogs e cobertura

- `content/machines_backlog.json`: **420** declarações de blocos com TE detectado, retiradas as bombas já atribuídas à outra frente. Registra classe/arquivo, TE, GUI, energia/fluido e multibloco. Os sinalizadores são inspeção estática/heurística, não prova de ausência quando falsos.
- `content/equipment_backlog.json`: **226** entradas de equipamento com comportamento especial; detonador fica no contrato de bombas.
- `tools/content-gen/bulk-coverage.json`: **1116 lacunas de declarações** e **159 registros ainda não classificados** (casts/aliases/expressões dinâmicas). Não some esses números como porcentagem: famílias metadata expandem em vários IDs e há sobreposição de registros/declaracões.
- Os upgrades simples, lâminas, cristais FEL, componentes de circuitos e peças com textura direta agora entram como identidades estáticas; seus efeitos continuam a cargo das máquinas. Entre as lacunas ficam UI de máquina, componentes com comportamento/NBT, fluidos dinâmicos cheios, extração de petróleo/gás, mineração especial, equipamentos especiais e modelos não representados. Células/recipientes legados que são itens simples entram como conteúdo; colocar/recolher fluidos e retornar recipientes não é implementado automaticamente por `iron_ingot`.
- Duas escadas de lightstone continuam sem textura resolvida pelo scanner (`lightstone_tile_stairs`, `lightstone_bricks_stairs`).

## Criativo no motor base

Não há campo de aba criativa no content API 0.2 deste checkout. `crates/horizon-mods/src/content.rs:78–86,113–119` usa `deny_unknown_fields` em BlockFile/ItemFile. `crates/horizon-client/src/creative.rs:190–204` lê exclusivamente os exports preparados `creative-inventory.json`/`creative-tabs.json`; `mod_content.rs:652–667` não mescla essas tabelas. O agente de pesquisa confirmou independentemente essas linhas. O agente creative-plus implementou a extensão em um fork separado. Esta entrega integra seu `tools/creative-tabs/generate.py` no fim do content-gen: gera **8 abas NTM**, cada um dos 1881 IDs em exatamente uma delas, e mescla títulos en_us/pt_br depois das traduções de itens/blocos. Um manifesto separado `.creative-tabs-files.json` limpa apenas tabelas antigas desse gerador; `lang-additions.json` fica em staging temporário. O motor base ignora `data/ntm/creative_tab/*.json` sem erro. **As abas exigem o patch creative-plus**: a aceitação visual no fork foi entregue ao agente 36a589a1. No motor base, o acesso continua por `/give` e receitas. Nenhum código do motor base foi alterado por esta frente.

## Verificação

- `cargo-horizon-mod build` e `check`: passaram; SHA acima.
- Regeneração determinística: **9214 arquivos**, zero diferenças, incluindo idiomas, tabelas criativas e tags de radiação (`regeneration.json`).
- `tools/content-gen/validate-bulk.py`: passou para todos os IDs, traduções sem placeholder, pais/modelos/texturas, loot, grafo de itens/tags/receitas, cobertura hazard e igualdade exata de tags por taxa/bit.
- Cliente base: `agent-run --user sacer`, Xvfb **:10**, cwd/HOME/dados/config em `/tmp/ntm-bulk-JcXS7U`. `HORIZON_NO_AUDIO=1`, options master/music 0.0, `tools/test-game/mute.py`. Log confirma áudio desligado; nenhum stream do cliente foi encontrado. O cliente foi encerrado normalmente e o Xvfb próprio foi removido.
- **135 comandos / zero falhas**: 21 amostras de setblock (Nether/End/metadata, concreto, vidro, escada/laje/pilar, bombas, fallout), 20 itens concedidos e visíveis, loot real de cobalto Nether, power/unpower da lâmpada-bomba, craft e retirada reais de 9 aço→bloco e 2 tijolos→4 placas de polímero. Não houve conteúdo recusado, textura/modelo ausente ou receita rejeitada no log. O primeiro ensaio leu estados antes da replicação; foi corrigido no harness com espera entre comandos, com o registro anterior preservado.
- Evidências: `content-gen-evidence/bulk/{static-validation,game-validation}.json`, `client.log`, `01-expanded-blocks.png`, `02-expanded-inventory.png`. O cliente carregou 3539 itens / 37844 estados totais (vanilla+mod); recursos levaram 110,7s nesta máquina/sessão.
- `cargo test --workspace` no catálogo ampliado encontrou 77 testes passando e 5 falhas da radiação M1 (contagem fixa de 13 hazards e timings de amostragem). O coordenador atribuiu a correção à fundação; os testes/timings não foram enfraquecidos nesta frente. Esse resultado deve ser reconciliado com a entrega posterior da fundação antes da aceitação conjunta.
- Não houve commit, instalação no diretório real de mods, uso de `~/.minecraft` ou alteração do diretório de dados permanente do usuário. `chown -R sacer:sacer /home/sacer/Workspace/ntm-port` aplicado.

## Reprodução

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
cd content
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod build
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod check target/horizon-mod/ntm.hmod
```

Para reauditar a fonte e reconstruir `bulk.json`: `agent-run python3 tools/content-gen/bulk.py --stage decorative`; `bomb_catalog.py` reconstrói o contrato de bombas. Os estágios materials/components produzem subconjuntos e destinam-se a lotes, não a reduzir um catálogo já usado num save. O teste de jogo usa `agent-run --user sacer bash tools/content-gen/run-bulk-client.sh`, depois `agent-run python3 tools/content-gen/verify-bulk-game.py` e o comando de controle `quit`.
