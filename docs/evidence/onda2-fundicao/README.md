# Fundição da Onda 2: provas em jogo (20/11/2026, revisão 3)

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
