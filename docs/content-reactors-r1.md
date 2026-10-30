# Reatores R1: pile atual e ZIRNOX

Entram 35 IDs, levando o catálogo a 2.835 (461 blocos e 2.374 itens). A pile legada fica excluída. Foram reaproveitados o catálogo de metadata, os tradutores de receitas/hazards, o visual-gen, o machine-gen e o probe de conteúdo dos lotes de energia.

| Grupo | IDs (prefixo `ntm:`) |
| --- | --- |
| Pile atual | `pile_brick`; `pile_device__loader`, `__vent`, `__control`; `pile_block__dummy`, `__core`, `__channel`, `__fuel_in`, `__fuel_out`, `__air_in`, `__air_out`, `__control`, `__edge` |
| Barras pile | `pile_rod__ra226be`, `__po210be`, `__zr`, `__nu`, `__pu239`, `__rgp`, `__waste`, `__thorium`, `__thorium_fuel` |
| ZIRNOX | `machine_zirnox`, `zirnox_destroyed`; 11 variantes `rod_zirnox__<EnumZirnoxType em minúsculas>`: natural_uranium_fuel, uranium_fuel, th232, thorium_fuel, mox_fuel, plutonium_fuel, u233_fuel, u235_fuel, les_fuel, lithium, zfb_mox |

As 14 máquinas/estruturas usam template `beehive`, facing horizontal e honey_level 0..5, com `auto: false`. O tijolo é bloco simples sem BE. Decisão do coordenador: as nove metas estruturais usam IDs distintos, preservando o papel sem sobrecarregar honey_level. O item `pile_device` tem metas 0/1/2, mas o bloco original usa 0/4/8 mais orientação; ambos constam no catálogo. As estruturas internas e o ZIRNOX destruído ficam ocultos no criativo e sem receita. Estruturas não dão loot; desmontagem/conversão em tijolos depende da lógica futura. O salvamento aleatório do ZIRNOX destruído também permanece lacuna.

São as texturas e traduções originais, com duas traduções pt_BR acrescentadas (barras de tório da pile). Dispositivos projetam seu atlas TESR no cubo de fallback; connected textures da pile, modelos, volumes e animações precisam da camada visual. O tijolo usa top/side e ainda não distingue norte/sul de leste/oeste como o fonte.

O snapshot `content/machine-recipes/reactors_simple.json` exporta as nove barras da pile (fonte de nêutrons, vida, reação, calor e transmutação), as 11 conversões ZIRNOX e o grafo original dos 24 slots. Preserva Th232→combustível de tório e Li→trítio; breeding não conta como vizinho ativo. A substituição ZIRNOX ocorre com `life>maxLife`, enquanto pile transmuta ao atingir a vida. Pile consome fluxo de saída no desgaste; o snapshot registra `BobMathUtil.squirt`, refrigeração por ar, temperaturas, construção e vínculos estruturais. ZIRNOX registra volumes dos tanques, ordem dos slots, pressão e conversão em vapor, incluindo a peculiaridade de não limitar o ciclo à água disponível/espaço livre. Nenhuma receita genérica de energia/calor foi inventada.

A vida é NBT int `life` no ZIRNOX e double `depletion` na pile; não usa `minecraft:max_damage`. Inserção, depleção, reação, calor, portas, telas, estruturas e falhas são trabalho nativo posterior. A declaração `beehive` fornece as propriedades do template; a instanciação do block entity no jogo nativo exige também registrar os IDs no bundle.

Receitas originais: +14 vanilla (11 cargas ZIRNOX e três dispositivos), total 974; montadora 455 (+cinco barras, tijolo e reator); bigorna 123 construções (+quatro barras); PUREX +quatro reciclagens de pile, preservando ingredientes e saídas. O perfil conservador PUREX passa de 23 para 27 operações; condições externas não foram habilitadas. Os demais quatro tipos de barra pile são produtos da reação, sem obtenção artificial. `addZIRNOXRod` foi expandido fielmente: casca existente e dois billets, com validação dos dois overloads originais. Não se incluiu o combustível genérico de pesquisa nem os itens pile legados.

Hazards: +35 identidades, 18 radioativas; nenhuma taxa nem outro hazard antigo mudou. O extrator passa a ler apenas o overload metadata de `registerOtherFuel` para `rod_zirnox`, exportando a taxa fresca e 11 lacunas explícitas de interpolação por vida. O lítio fresco é zero no fonte e tem alvo não zero ao depletar; não foi substituído por uma taxa inventada. As oito barras pile radioativas já usam registros ItemStack/enum resolvíveis. Tags e buckets foram regenerados. O responsável pela lógica aprovou hazards/buckets e as três tabelas regeneradas assembly/anvil/PUREX; `machine_files.json` permaneceu intacto.
