# Fundição da Onda 2: provas em jogo (20/11/2026, revisões 3 e 4)

Binário release `3fb561ff3b54…` (motor `onda2-writer-barrier` `7f61a9c171`, HEAD `27f31677`/`134a7ccf`: portas com direção, pilha
por item, pá e molde na mão) e `ntm.hmod` `6a380ecbafe7…`; Xvfb mudo, `HORIZON_NO_EAGER_SAVE=1`. Roteiros
`tools/test-game/native_foundry_cut.py` e `run-foundry-proof.py` (a máquina de cortes do `run-crash-proof.py`), contagem em quanta de
ouro por `foundry_quanta.py`. Cada ponto joga o mundo do mod e o controle vanilla (barris), corta o jogo no ponto e reabre duas
vezes. As máquinas são lidas do servidor (`data get block ... ntm`). Relatórios completos em `report-*.json`.

## Cenários

- **scrap** (D4): cadinho com 96 quanta de ouro quebrado, a sucata exata de volta num slot.
- **caster** (F1+F4): o jato de um cadinho cai na célula do funil; poço de 144; quebra: molde e sucata de 144.
- **excess** (F5 + P1-1): o excesso de 64 de um molde mais leve sai como uma sucata exata pelo ledger; o corte `kv_committed`; o
  molde sobrevive à reabertura.
- **batch** (F4 com água): um barril de aço no primeiro porto do lingotamento, enchido com um balde pela tela e posto em saída;
  os 2 lingotes do cadinho caem e são **fundidos** (2 lingotes nas saídas, poço 0, água 1000→280 mB, vapor gasto 720 mB); quebra.
- **hand** (pá e molde na mão): sem o molde (tirado por comando, a chave de fenda não existe), um clique com molde na mão o põe
  no slot 0 (um da pilha de 2) e um clique com pá leva o poço inteiro: uma sucata `{ntm_amount:144}` no inventário, poço 0.

## Resultado: 40 de 40 pontos passam, todos com o mesmo binário (8 pontos por cenário, 5 cenários)

| Ponto | scrap | caster | batch | hand | excess |
|---|---|---|---|---|---|
| none, kill, save_begin, world_saved, players_saved | 96 | 144 | 144 | 144 | 144 |
| kv_committed | 96 | **288** | **288** | **288** | **208** |
| chunks_written, entities_written | 96 | **0** | **0** | **0** | **80** |

(disco = 1ª reabertura = 2ª reabertura = controle vanilla, em todas as células.) Onde o número difere, o controle vanilla tem o
mesmo, e as razões são as do corte, não do mod:
- `kv_committed`: o arquivo do jogador (com o que recolheu) é gravado antes de a região reter o bloco/pool: uma cópia que o
  vanilla mostra igual.
- `chunks_written` e `entities_written`: o corte deixa gravados os chunks sem o bloco e ainda sem o arquivo do jogador: a perda é
  a do vanilla. No `excess` o que sobra gravado é o poço de 80.

## O corte em jogo do P1-1 NÃO distingue o defeito (declarado)

`excess/kv_committed` passou também com o binário antigo (`757b39c2ad08`, sem a correção `slots_recorded`): em jogo esse corte não
reabre o lingotamento a partir do journal. O defeito e a correção estão provados no teste de servidor
`a_caster_restored_from_the_journal_keeps_its_mold_and_outputs_and_drops_the_excess_once` (vermelho sem a correção, verde com
ela; o mutante `P1-1` o mata).

## O que não foi provado em jogo

- `rotary-pour` (vapor por dutos) e a borda de chunk R6: só teste de servidor (lacuna aprovada). A cadeia rotativo → lingotamento
  → lingotes tem teste de servidor (`a_rotary_jet_on_a_funnel_cell_fills_the_caster_and_it_casts_the_batch`).
- A queda da sucata em `y+maxY` e a chave de fenda (`onScrew`): fora (a chave não existe no conteúdo).

## Mutantes

`mutants-foundry.log`: 34 de 34 mortos (`tools/test-game/mutants_foundry.py`): F1, F4, F5, P1-1, D4, R8, o núcleo do rotativo, as
direções dos portos, o tamanho de pilha e a pá e o molde na mão.

## Relançar

    HORIZON_NO_EAGER_SAVE=1 agent-run python3 tools/test-game/run-foundry-proof.py --scenario scrap|caster|excess|batch|hand \
        --binary B --resources R --bundle H --scratch-root S --keep [POINT ...]

## Revisão 4: a fundição por clique (binário `cb6e15de0658…`, HEAD `62f9b62d`)

O gancho de mão do lingotamento virou o da fundição inteira (`foundry_hand.rs`: molde e bacia, canal, tanque, cadinho, lingotamento; só a
mão principal; as pás vêm do conteúdo). Provas em jogo com esse binário (reports `report-foundry-clicks-*.json`,
`report-foundry-hand-x_eu4p9u.json`; os quatro `hand` anteriores sem pontos foram tentativas que o mundo de controle derrubou por
carga da máquina, sem nada a ver com o código):

- **hand** (o lingotamento por clique, agora pelo gancho geral): 8 de 8 pontos, 144 em todos (kv_committed 288, chunks/entities_written 0),
  iguais ao controle. O molde sai à mão **pela tela** do lingotamento (slot 0) e volta por um clique com molde na mão: o molde não fica
  preso sem a chave de fenda.
- **clicks** (novo): um cadinho com 144 quanta de ouro cuja corrente cai sobre um bloco de molde sem molde (nada verte); como jogador em
  sobrevivência: um clique na face de cima com um molde na mão o coloca (um da pilha de 2); a corrente enche o molde e o lingote
  esfria no slot; com o lingote no slot nada mais verte, e uma pá no cadinho leva os 72 que sobraram como uma sucata; um clique
  com a mão vazia tira o lingote. 8 de 8 pontos, mesmos números do controle: 144 (kv_committed 288, chunks/entities_written 0).

Os cenários scrap, caster, excess e batch da revisão 3 não foram repetidos com este binário (o que mudou nele é só o gancho de mão).
Mutantes: `mutants-foundry.log`, 41 de 41 mortos.

## Revisão 5: a bica (`ntm:foundry_slagtap`) e a escória (`ntm:slag`) (rodada reexecutada com o binário final `dde029c5ca87…`, `ntm.hmod` `59181d49dcc8…`; o `commit_machines` agora marca as células como próprias)

Commits: `80ba2f30` (a bica, a escória, o commit único, testes de servidor, conteúdo) e `6ce9aace` (mutantes, roteamento, roteiros de jogo).
O vazamento é **um commit só** (`commit_machines` com o registro menor da origem, o maior da escória atingida e o bloco novo com o
primeiro registro, `CellWrite.initial`): recusado, nada muda; aceito, os três chegam juntos. **Um chunk só**: se a escória sairia do chunk da
origem, a bica não vaza e a origem guarda o metal (o diário de vários chunks não sabe recriar o bloco que falta numa gravação rasgada).

Provas em jogo (reports `report-foundry-slagtap-*.json`), 144 quanta de ouro (2 lingotes) num canal, a bica posta por comando e **a
gravação pedida em seguida** (o vazamento corre contra ela), cortes estritos, contra o controle vanilla (barris):

- **slagtap-pour**: 8 de 8 pontos, **144 em todos**, 0 perdido, nenhum duplicado, igual ao controle, mesmo corte. O mundo reaberto
  (evidência `where`) está no estado de antes do vazamento (`kill`, `save_begin`: 144 no canal, sem bloco de escória) ou no de depois
  (`none`, `kv_committed`, `chunks_written`, `entities_written`, `world_saved`, `players_saved`: 0 no canal, 144 no bloco `ntm:slag`),
  nunca nos dois e nunca em nenhum.
- **slagtap-border** (canal em z 15, bica e fosso em z 16: outro chunk): a bica **não vaza** e o canal fica com os 144 em todos os pontos
  (`where`: canal 144, sem bloco). Todos os 8 pontos: 144, 0 perdido, nenhum duplicado, não pior que o controle. `ok` do relatório é falso
  em 4 pontos (`chunks_written` em diante) só porque a máquina de cortes não casou o conjunto de chunks gravados com o do controle
  (`same_cut: false`): o controle mexe nos barris dos dois chunks, o mod não muda nada (não vaza). Os números não dependem disso.
- **slagtap-break** (D4): o jogador em sobrevivência quebra o bloco de escória de 144 e a sucata exata (`ntm_amount:144`) chega ao inventário
  (8 de 8 pontos ok). **Atenção**: em `chunks_written` e `entities_written` o mundo reaberto tem **0** (144 perdidos) e em `kv_committed`
  **288** (144 duplicados): são os mesmos números do controle vanilla (`control_lost` 144, −144), o resíduo da ordem em que o arquivo do
  jogador é gravado depois dos chunks (o mesmo de `scrap`, `hand` e `clicks`, rodadas 3 e 4). Não vêm do vazamento (a escória já existia).

Mutantes: `mutants-foundry.log`, os 16 da escória e da bica, **14 mortos**. Dos dois que sobreviviam, (1) "um commit recusado
é vertido em memória mesmo assim" **não é equivalente** (revisão): o teste antigo usava `refuse_commits`, que recusa tudo, inclusive o commit
de uma máquina só do `native_save`; na vida real a recusa pode atingir só o vazamento (o jogador põe um bloco na célula de cima no mesmo tique)
e o `native_save` publicaria a origem reduzida sozinha. O teste novo `a_pour_refused_alone_...` recusa só os commits com células ou de várias
máquinas (`refuse_structured`) e o mutante morre. Resta um equivalente: (2) "toda célula de cima é ar": a definição do raio já garante que a célula de cima da
primeira atingida é ar, uma máquina (excluída antes) ou fora do chunk (excluída antes).

Lacunas declaradas: sem o escoamento da escória, cor neutra, sem o vazamento entre chunks, mato/neve/líquido não são substituídos, sem
fechamento por redstone, sem partícula do jato, sem pick-block (`docs/content-foundry.md`).

## Onde o metal está depois de cada corte (`where-slagtap.json`, binário `c8782234102c…`)

Versionado a partir das pastas de prova. Canal / escória, em quanta, no mundo reaberto:

| cenário | none | kill | kv_committed | save_begin | chunks_written | entities_written | world_saved | players_saved |
|---|---|---|---|---|---|---|---|---|
| slagtap-pour | 0 / 144 | 144 / 0 | 0 / 144 | 144 / 0 | 0 / 144 | 0 / 144 | 0 / 144 | 0 / 144 |
| slagtap-border | 144 / 0 | 144 / 0 | 144 / 0 | 144 / 0 | 144 / 0 | 144 / 0 | 144 / 0 | 144 / 0 |
| slagtap-break | 0 / 0 (sucata no inventário) | 0 / 144 | 0 / 144 (+ inventário: 288) | 0 / 144 | 0 / 0 (0 no total, vanilla) | 0 / 0 (idem) | 0 / 0 (inventário) | 0 / 0 (inventário) |

## Fechamento por redstone (`slagtap-redstone`, ponto `none`, mesmo binário)

Um bloco de redstone ao lado da bica a fecha: depois de 15 s o canal guarda os 144 e não há bloco de escória; tirado o bloco, a bica abre e os 144
vão para a escória (`ntm:slag`); 144 no mundo reaberto duas vezes, igual ao controle. A inversão (`invert_redstone`) e a leitura ausente
ficam nos testes de servidor (`redstone_closes_*`), cinco mutantes mortos.

## Revisão da revisão: o escopo das células próprias

`commit_machines` marcava como próprias **todas** as células dos seus commits; o passo local põe nele os `world_writes` da mineração
(escavadeira e broca), então um cabo minerado não gerava `Broken` e ficava fantasma. Agora só as células com `initial` (a máquina que nasce
junto, a escória) são próprias: `ntm_core::server::owned_cells`, com teste (`only_the_cells_born_with_a_machine_are_owned`) e mutante morto.
Os cenários `slagtap-*` foram reexecutados com o binário depois desta correção (abaixo, mesma tabela). O `mutants-foundry.log` anexa as
execuções novas: os 5 de redstone, `cave_air`, o do commit recusado só no vazamento (agora morto) e o das células próprias.

## A escavadeira minando um cabo (`run-mining-cable-game.py`, binário `c8782234102c…`)

Uma bateria que dá, cinco `red_cable` e uma bateria que recebe; a `power` da segunda, lida do **modelo do servidor** (`data get block … ntm`),
sobe (111 622 → 323 204 em 6 s). A escavadeira (bateria criativa e broca de ferro) minera os três cabos do seu alcance (as células viram ar),
a `power` da bateria que recebe **para de subir** (614 754 → 614 754 em 8 s) e os dois cabos de fora ficam. 8 de 8 verificações
(`report-mining-cable-qbi0vyhe.json`). **Honestidade**: o mesmo cenário também passa com o binário da rodada intermediária (`dde029c5ca87…`),
que tinha o escopo largo demais: o cabo fantasma, na prática, é refeito pela rede em poucos segundos, então o cenário não separa o defeito
antigo; o que o garante é o teste do core (`only_the_cells_born_with_a_machine_are_owned`) e o mutante. O outro lado de `owned_cells`
(a máquina que nasce junto) é o `slagtap-pour` com este mesmo binário: a escória guarda os 144 e não vira máquina vazia.
