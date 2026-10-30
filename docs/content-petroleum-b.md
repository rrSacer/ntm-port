# Petróleo, lote B: refinaria, fração e mistura

Este lote acrescenta `machine_refinery`, `machine_fraction_tower` e `machine_mixer`, todos no contrato de máquinas beehive/facing/honey_level 0..5. Entram também `steel_scaffold`, `steel_beam`, `steel_grate` e `barricade`. O catálogo chega a **2.646 IDs**: 382 blocos, 2.264 itens e 45 máquinas. Os 157 fluidos existentes bastam.

Nomes en_us/pt_br, texturas NTM, loot, hazards e abas foram regenerados. O visual-gen conserva seus arquivos de modelos, telas e multiblocos; o cubo opaco das máquinas é o fallback para o motor base. A aprovação deste conteúdo não implementa a lógica do petróleo.

## Receitas e escolha

`tools/content-gen/petroleum_refining.py` exporta schema 1 em `content/machine-recipes/`, na ordem das declarações. Todos os registros têm fonte, índice original, entradas, saídas, labels pt_br/en_us e configuração do tile com fontes.

| JSON | Chamadas na fonte | Alternativas | Exportadas | Skipped |
| --- | ---: | ---: | ---: | ---: |
| refinery.json | 4 | 4 | 4 | 0 |
| fraction.json | 19 | 19 | 19 | 0 |
| mixer.json | 40 | 50 | 49 | 1 |

A refinaria escolhe pelo fluido de entrada: **100 mB por ciclo, um ciclo por tick, 5 HE por ciclo**, buffer de 1.000 HE, tanque de entrada de 64.000 mB e quatro tanques de saída de 24.000 mB. Os quatro valores de fração são quantidades por 100 mB. Exemplo de óleo quente: 50 pesado, 25 nafta, 15 leve, 10 petróleo.

O sólido não sai a cada ciclo. Há **um contador sulfur por máquina**, salvo em NBT e preservado ao mudar receita ou fluido; a cada dez ciclos bem-sucedidos ele tenta inserir o sólido. O JSON conserva `outputItems:[]` e usa `periodicOutputItems:[{item:[id,count],every:10,counter:"sulfur",source:...}]`. É uma regra determinística, não probabilidade. Se o slot de saída estiver cheio ou incompatível, a fonte perde aquela saída sólida e continua o refino; isso também está documentado no contrato. O agente de lógica aprovou esse campo antes da extração.

A torre de fração processa **100 mB a cada dez ticks**, sem consumo de HE. Seus três tanques têm 4.000 mB. Inclui as 19 linhas completas, não só petróleo: EGG, mistura de clorocalcita e solução de bauxita também estão presentes. COALOIL produz 30 mB de COALGAS e 70 mB de OIL.

O misturador usa **50 HE/t**, buffer de 10.000 HE, dois tanques de entrada de 16.000 mB e saída de 24.000 mB. O tempo é o de cada alternativa. A seleção é pelo fluido de saída escolhido e pelo índice da alternativa, módulo a quantidade disponível; `source_index` e `alternative_index` conservam os dois níveis da declaração. As entradas dos tanques permanecem ordenadas. Entram as quatro rotas de solvente (nafta normal, craqueada, dessulfurizada ou de coque + aromáticos) e a rota de radiosolvente (750 reformgas + 250 cloro).

A única alternativa omitida é PHEROMONE_M, que exige o item ausente `pill_herbal`, previsto para o lote E. A identidade de bauxita existente foi resolvida pela qualificação `BlockEnums.EnumStoneType`, sem criar ID. Peixe cru legado conserva quatro alternativas; double_plant metadata zero corresponde a sunflower.

## Construção, estruturas e barricade

`ass.refinery` entra na montadora, que passa para 419 receitas. Torre e misturador têm as receitas de bancada originais, assim como andaime, viga e grade. O snapshot vanilla já resolvia KEY_ANYPANE: não foi necessário criar um item ou alias de paneGlass.

O resolvedor de bancada passou a traduzir os métodos fonte `plateCast()` e `plateWelded()` para as chaves MaterialShapes `plateTriple` e `plateSextuple`. Isso recupera a receita da torre e outras receitas estáticas originais. O datapack passa de 891 para **905 receitas vanilla**, com seis tags de alternativas de placas. Nenhuma ponte temporária foi criada.

As estruturas usam textura NTM e cubo de fallback. A geometria fina, o posicionamento variável da grade e as quatro cores/metadata do andaime ainda não estão representados; o catálogo registra essa lacuna. A textura-base do andaime é `scaffold_steel`, conforme seu registro de ícones, e a grade usa `grate_top` como fallback.

`ntm:barricade` serve às paredes da escavadora. Usa textura barricade, dureza 1, resistência 2,5, sem exigência de ferramenta, **loot vazio, sem receita e fora das abas criativas**. Um item de comando é conservado pelo pipeline de blocos, mas `creative_hidden:true` impede sua inclusão nas abas. A validação exige cobertura única somente dos IDs visíveis, e verifica os IDs omitidos explicitamente.

## Verificação

Passaram **95 testes Python em cada seed, PYTHONHASHSEED=1 e 42**, incluindo nove testes do lote B; passaram também `validate-bulk.py` e `tools/recipe-gen/validate.py`. A suíte confere as 50 alternativas, o contador periódico, parâmetros, identidades, IDs/fluidos, construção, ausência de loot/aba/receita da barricade e regeneração em diretório novo. Hazards, tags de radiação (97 taxas/27 bits) e oito abas foram regenerados.

Build/check API 0.2 passou em pacote temporário no disco: **5.538.048 bytes**, SHA-256 `d84fa017a0a02f43987bc526cd66ab7f90b5a618e59c86b903a874935f42a83f`. O pacote fica em `../.agent-tmp/ntm-petroleum-b-content-e04wfwxn/target/horizon-mod/ntm.hmod`; não foi instalado no jogo real. O agente de lógica acrescentou os três registros Inert em `7f7a424` e confirmou `cargo test --workspace` e clippy com `-D warnings` no HEAD isolado por git archive, usando disco em `.agent-tmp`; o teste de concordância do registro passou. Evidências locais: `content-gen-evidence/petroleum-b/`. Não há worldgen neste lote.
