# Fundição da Onda 2: provas em jogo (estado em 19/11/2026, pausa pedida pelo usuário)

Binário release `onda2-fundicao-native` (motor `onda2-writer-barrier` `7f61a9c171`, `ntm.hmod` do HEAD `a206ed5f`+),
`HORIZON_NO_EAGER_SAVE=1`, roteiros `tools/test-game/run-foundry-proof.py` (contagem em quanta de ouro por
`foundry_quanta.py`).

## O que rodou

| Cenário | Ponto | Resultado |
|---|---|---|
| scrap | none | passa: 96 quanta no disco, na 1ª e na 2ª reabertura, e no controle (barril com 1 lingote e 3 pepitas de ouro) |
| scrap | kill | passa: o mesmo |
| scrap | kv_committed | interrompido pela pausa |
| scrap | save_begin, chunks_written, entities_written, world_saved, players_saved | não rodaram |
| caster | prepare (mundo mod) | passa: o cadinho derrete 2 lingotes e o jato cai na célula do funil; o poço do lingotamento fica com 144 quanta e o cadinho com 0 (F1+F4 no motor real) |
| caster | none | falhou no `act`: o roteiro não quebrou o lingotamento (as células dummy ocultam o núcleo; corrigido no roteiro: quebra o dummy que está no caminho) |

A primeira rodada de `scrap/none` com a contagem antiga deu 168 em vez de 96 (o contador somava a sucata do slot duas
vezes, uma pelo `components` do bloco e outra pelo `custom` do modelo): defeito do contador, corrigido em `51318d50`.

## O que relançar

    HORIZON_NO_EAGER_SAVE=1 agent-run python3 tools/test-game/run-foundry-proof.py --scenario scrap --binary B --resources R --bundle H --scratch-root S --keep
    ... --scenario caster ...

Os jogos eram lentos por memória (a `user-0-builds.slice` em 8G com compilações de outros agentes); com 12G e a máquina
calma um ponto leva cerca de 10 min. O `rotary-pour` em jogo é uma lacuna declarada (aprovada pelo coordenador).
