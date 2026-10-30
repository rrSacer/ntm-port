# Roteiros de aceitação no jogo nativo

Os roteiros novos usam o jogo com NTM e Recipes de `tools/build-native.sh`,
num mundo criativo de teste. Todos os jogos e builds passam pelo `agent-run`,
em Xvfb mudo e com homes descartáveis. Nunca use um mundo real.

## Prensas

Com o jogo aberto e `control.sock` na pasta de teste:

```sh
python3 tools/test-game/native_presses.py /caminho/do/jogo prensa eprensa
```

O driver usa o socket diretamente, grava cada comando e resposta em
`native-presses.json` e sai com erro se uma conta falhar. `native_game.py`
implementa a espera por jogador/chunks, inserção por cliques reais e a posição
absoluta de botões de `horizon_screen`; não altera estado interno de máquina.
Para multiblocos, a prensa é aberta pelo dummy superior visível.

| Fase | Conta preservada de `presses.py` |
| --- | --- |
| prensa | 8 lingotes → 8 chapas; matriz com desgaste 8; 8 chapas retiradas ao inventário |
| eprensa | matriz de fio com desgaste 62 + 5 lingotes de cobre → 16 fios; sobram 3 lingotes e a matriz quebra |

A elétrica recebe uma bateria criativa no slot 0. Isso isola o processamento
da aceitação das redes HE, a cargo de `native_redes.py`.

### Prova de 30/10/2026

Motor `9803b72cd`, bundle NTM `db859b71`, Recipes oficial `ae217daa`, conteúdo
fixado da L2 (`ntm-l2c.hmod`). Binário fast isolado da prova V5; o novo build
release de `build-native.sh` foi interrompido antes de terminar, portanto esta
rodada não comprova esse build release.

- **Prensa passou:** 8 chapas no inventário, desgaste 8 da matriz.
- **Elétrica falhou, pendência enviada ao responsável pela lógica:** os três
  itens entram nos slots, mas a bateria criativa não carrega a máquina.
  Após 180 segundos: energia 0/50.000 HE, matriz ainda com desgaste 62,
  cinco lingotes e nenhum fio. O roteiro preserva o erro, sem afrouxar a conta.
- **Centrífuga a gás:** a entrada nativa será testada por barril e dutos UF6.
  O botão antigo da tela WASM não representa um slot nativo. O responsável
  confirmou que o slot 5 é identificador, o 6 é upgrade e o tanque inicia UF6.
  Mudar o modo do barril exige a correção de botões `7b56d7d7` ou posterior.

Evidência local: `.agent-tmp/c65d-native-tests/runs/game-7gx97fwq/`, no diretório
pai dos repositórios. `native-presses.json` conserva os comandos e checks;
`native-presses-prensa.png` mostra a saída e o desgaste;
`native-presses-eprensa-stalled.png` mostra os slots parados. O compound da
elétrica está também em `.agent-tmp/c65d-native-tests/epress-data.json`.

## Continuação

Fundição, bigorna, triturador, centrífuga, bedrock, petróleo A–D, purex e m3
continuam em adaptação. Falhas de lógica são encaminhadas ao responsável por
`ntm-core`; os roteiros não corrigem `crates/` nem injetam estado para passar.

## Fundição nativa

`foundry.py` aceita `HORIZON_CTL` (o ctl do build), prepara chunks e chão antes
do teleporte e preserva as fases `fundir`, `canais`, `reter`, `voltar`.
O setup limpa do inventário somente o item de entrada antes de dar o lote:
sem isso o shift-click realimentava a máquina com lingotes recuperados na
fase anterior, invalidando a conta de entrada.

Prova em Xvfb mudo, server-thread, com o mesmo fast/V5 e bundle fixados acima:

| Fase | Resultado |
| --- | --- |
| fundir | 3 lingotes no cadinho → 3 retirados; inventário 3; ~65 s |
| canais | 2 lingotes novos → 2 retirados após canais/tanque/saída; ~25 s |
| reter | Molde retirado, 3 lingotes novos, tanque com metal e inventário 0; salvo |
| voltar | Mundo copiado para outra pasta descartável e reaberto; molde recolocado → 3 lingotes recuperados, inventário 3; ~43 s |

As contas de saída são assertivas: qualquer quantidade diferente da esperada
encerra o roteiro com erro. `py_compile` e `git diff --check` também passaram.
Nenhum `took too long` nos logs.

Evidência da sequência exata: `.agent-tmp/c65d-native-tests/runs/game-wn3b0ua3/`;
reabertura em `.agent-tmp/c65d-native-tests/runs/game-vknf39zk/`.
Cada pasta contém `foundry-evidence.json`, `client.log` e capturas.
