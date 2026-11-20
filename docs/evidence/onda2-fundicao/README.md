# Fundição da Onda 2: provas em jogo (20/11/2026, revisão 2)

Binário release `447544abf8d6…` (motor `onda2-writer-barrier` `7f61a9c171`, HEAD `70b8a69e`, com a correção do P1-1) e
`ntm.hmod` `6a380ecbafe7…`; Xvfb mudo, `HORIZON_NO_EAGER_SAVE=1`. Roteiros `tools/test-game/native_foundry_cut.py` e
`run-foundry-proof.py` (a máquina de cortes do `run-crash-proof.py`), contagem em quanta de ouro por `foundry_quanta.py`. Cada
ponto joga o mundo do mod e o controle vanilla (barris), corta o jogo no ponto e reabre duas vezes. As máquinas são lidas do
servidor (`data get block ... ntm`), não da visão do cliente. Relatórios completos em `report-*.json`.

## Cenários

- **scrap** (D4): cadinho com 96 quanta de ouro (lingote e 3 pepitas) quebrado, a sucata exata (`ntm_amount:96`) de volta num slot.
- **caster** (F1+F4): o jato de um cadinho cai na célula do funil; 2 lingotes (144) no poço; quebra: molde e sucata de 144.
- **excess** (F5 + P1-1): o poço de 144 e, por um molde mais leve (capacidade 80), o excesso de 64 sai como uma sucata exata
  pelo ledger; o corte `kv_committed` reabre o chunk com o registro do journal. Controle: barril com 1 lingote e 1 pepita e
  8 pepitas na mão do jogador. A verificação confere que o molde do lingotamento sobrevive à reabertura.

## Resultado: 24 de 24 pontos passam com o binário novo (8 por cenário)

| Ponto | scrap | caster | excess |
|---|---|---|---|
| none, kill, save_begin | 96 | 144 | 144 |
| kv_committed | 96 | **288** | **208** |
| chunks_written, entities_written | 96 | **0** | **80** |
| world_saved, players_saved | 96 | 144 | 144 |

(disco = 1ª reabertura = 2ª reabertura = controle vanilla, em todas as células.) Onde o número difere de 96/144, o controle
vanilla tem o mesmo, e as razões são as do corte, não do mod:
- `caster/kv_committed` 288 e `excess/kv_committed` 208: o arquivo do jogador (com o que ele recolheu) é gravado antes de a
  região reter o bloco/pool: uma cópia (144 e 64) que o vanilla mostra igual.
- `caster/chunks_written` 0 e `excess/chunks_written` 80: o corte deixa gravados os chunks sem o bloco e ainda sem o arquivo do
  jogador: a perda (144 e 64) é a do vanilla.
- Uma tentativa de `excess/players_saved` falhou no roteiro (a sucata já estava no inventário, nenhum item no chão a pegar):
  corrigido no roteiro, a tentativa seguinte passou (`report-*` mantém as duas).

## O corte em jogo do P1-1 NÃO distingue o defeito (declarado)

`excess/kv_committed` passa com o binário **antigo** (`757b39c2ad08`, sem a correção) e com o novo: o molde sobrevive nos dois.
Em jogo o corte `kv_committed` não reabre o lingotamento a partir do journal (o lote do excesso se assenta antes), então o
cenário não reproduz o P1-1. O defeito e a correção estão provados no teste de servidor
`a_caster_restored_from_the_journal_keeps_its_mold_and_outputs_and_drops_the_excess_once` (vermelho sem `slots_recorded`,
verde com ele; o mutante `P1-1` do `mutants-foundry.log` o mata). O cenário em jogo continua útil como prova de que o excesso
sai uma vez e de que nada se perde nem se cria nos 8 cortes.

## O que não foi provado em jogo

- `rotary-pour` (vapor por dutos) e a borda de chunk R6: só teste de servidor (lacuna aprovada).
- O lote do lingotamento com água em jogo: teste de servidor.

## Mutantes

`mutants-foundry.log`: 22 de 22 mortos (script `tools/test-game/mutants_foundry.py`): F1, F4, F5, P1-1, D4, R8 e o núcleo do
rotativo (steamUseMult, canProcess ×2, consumo, retorno de vapor, meia queima, vazamento de 72, posição do bico, velocidade).

## Relançar

    HORIZON_NO_EAGER_SAVE=1 agent-run python3 tools/test-game/run-foundry-proof.py --scenario scrap|caster|excess \
        --binary B --resources R --bundle H --scratch-root S --keep [POINT ...]
