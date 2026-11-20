# Fundição da Onda 2: provas em jogo (20/11/2026)

Binário release (`horizon-client` sha256 `757b39c2ad08…`, motor `onda2-writer-barrier` `7f61a9c171`), `ntm.hmod`
(`6a380ecbafe7…`) do HEAD `1ab0e137`, Xvfb mudo, `HORIZON_NO_EAGER_SAVE=1`. Roteiros `tools/test-game/native_foundry_cut.py`
e `run-foundry-proof.py` (a mesma máquina de cortes do `run-crash-proof.py`), contagem em quanta de ouro por
`foundry_quanta.py`. Cada ponto joga o mundo do mod e o controle vanilla (barris com os mesmos lingotes e pepitas), corta o
jogo no ponto e reabre duas vezes. Os relatórios completos estão em `report-*.json` (5 corridas, 16 pontos).

## Cenários

- **scrap** (D4): um cadinho em fogareiro quente funde 1 lingote e 3 pepitas de ouro (96 quanta, uma quantia que
  nenhuma contagem de lingotes guarda); o ato quebra o cadinho como jogador em sobrevivência, recolhe o que cai (uma
  sucata `ntm:scraps__gold` com `ntm_amount:96`) e a põe de volta num slot do cadinho pela tela. Controle: barril com o lingote e as
  pepitas, quebrado, recolhido e devolvido.
- **caster** (F1+F4+F5): um lingotamento cuja célula do funil recebe o jato de um cadinho; 2 lingotes de ouro (144)
  fundem, caem e ficam no poço (sem água, nada funde); o ato quebra o lingotamento e recolhe o molde e uma sucata com os 144.
  Controle: barril com o molde e os 2 lingotes. A prova do preparo já mostra o jato pelo funil: poço 144, cadinho 0.

## Resultado: 16 de 16 pontos passam (8 por cenário)

| Ponto | scrap: disco / 1ª / 2ª reabertura / controle | caster: disco / 1ª / 2ª / controle |
|---|---|---|
| none | 96 / 96 / 96 / 96 | 144 / 144 / 144 / 144 |
| kill | 96 / 96 / 96 / 96 | 144 / 144 / 144 / 144 |
| kv_committed | 96 / 96 / 96 / 96 | **288** / 288 / 288 / 288 |
| save_begin | 96 / 96 / 96 / 96 | 144 / 144 / 144 / 144 |
| chunks_written | 96 / 96 / 96 / 96 | **0** / 0 / 0 / 0 |
| entities_written | 96 / 96 / 96 / 96 | **0** / 0 / 0 / 0 |
| world_saved | 96 / 96 / 96 / 96 | 144 / 144 / 144 / 144 |
| players_saved | 96 / 96 / 96 / 96 | 144 / 144 / 144 / 144 |

O disco e as duas reaberturas coincidem em todos os pontos (nada se perde nem se cria por reabrir) e a conta é a do controle
em todos. **Duas coisas honestas no `caster`, as duas iguais ao controle vanilla:**

- `kv_committed`: 288 em vez de 144 nos dois mundos. O corte grava o arquivo do jogador (com o que ele recolheu) antes
  de a região reter o bloco quebrado: a cópia que o vanilla também mostra no mesmo corte. O mod não faz pior que o controle,
  mas é uma duplicação de 144 que existe nos dois.
- `chunks_written` e `entities_written`: 0 nos dois mundos. O corte deixa gravados os chunks sem o bloco, mas ainda não o
  arquivo do jogador com a sucata recolhida: a perda de 144 também é a do controle (quem quebra um baú e perde o processo no meio
  do salvamento).

No `scrap`, onde o jogador põe a sucata de volta num slot antes do corte, nada disso aparece.

## O que não foi provado em jogo

- `rotary-pour` (o rotativo precisa de vapor por dutos) e o corte da borda de chunk (R6): lacuna declarada e aprovada. O
  vazamento do rotativo em molde, bacia e funil tem teste de servidor, e o jato do cadinho pelo funil foi provado aqui.
- A cadeia completa com a fundição em lote (precisa de água por dutos): o poço enche, mas o lote não foi visto em jogo;
  tem teste de servidor (`a_rotary_jet_on_a_funnel_cell_fills_the_caster_and_it_casts_the_batch`).
- A queda da sucata de excesso (F5) em jogo: provado só no servidor (`the_excess_after_a_lighter_mold_…`).

## Relançar

    HORIZON_NO_EAGER_SAVE=1 agent-run python3 tools/test-game/run-foundry-proof.py --scenario scrap|caster \
        --binary B --resources R --bundle H --scratch-root S --keep [POINT ...]

Em 19/11 os jogos eram estrangulados por memória (a `user-0-builds.slice` em 8G com compilações de outros agentes); com o
limite maior e a máquina livre um ponto leva cerca de 10 minutos.
